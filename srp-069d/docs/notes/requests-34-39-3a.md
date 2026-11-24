# Requests 34 00, 39 00 and 3A 00 (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read in the 0.69d exe (address given); LIKELY = inferred; CHOICE = a server rule with no client
evidence; OPEN = no evidence in the exe, the game data or the notes read.

All three are `u8 00` bodies built by the generic builder 0x200185FE0 (the id is its argument, call site 0x200185FF6). The
protocol crate decodes them as `Request::Generic { id, a }`. Unlike the pending-flag requests (5B 01/02, 2F, 62 23) none
of them sets a "waiting for the server" flag, so a missing answer never blocks the UI.

## 3A 00: asks the faction unlock mask (PROVEN trigger, reply PROVEN by exclusion)

* Sender: `lobby_menu::important_message_arrived` (0x2001C2140), message type 4 whose text starts with `#ru` (reputation
  update, string 0x20096A388). Text layout read by the code (0x1C30F1..0x1C3188): `#ru[<faction>:<level>][<int>][<int>]`;
  the two trailing ints are handed to 0x1C1F90, the premium proposal of the level-up (see notices.md). The handler builds
  the Flash message (`root.add_important_message` with `id`, `type`, `level`, `faction_id`, `faction_current`,
  `faction_reputation_delta`, the faction's `unlock_message` from `factions_dict`).
* Condition (0x1C31AB..0x1C31D8): `3A` is sent only when bit `faction-1` of the client's faction unlock mask (client state
  `L+0x7A3940`) is CLEAR. Then `35 00` is always sent (0x1C31F9, see below).
* Reply: the only writer of the mask is the `96` handler (0x20018ECC1, `u8 mask`; the other uses are a constructor clear
  at 0x183785 and readers 0x1C19B4 / 0x1C31B4). So the answer is a `96`.
  The client uses the mask in 0x2001C1980 (`root.setup_player_progress`): a set bit gives an empty faction text, a clear bit
  the `st_faction_*_blocked_text` string (table 0x200967580).
* Implemented: `3A 00` -> `96 <mask>`; the same function builds the sign-in `96` (`login::faction_mask_push`).
* CHOICE / OPEN: the mask is 0x0F (all four factions open). No file or exe string states how a faction becomes
  unlocked (searched: `reputation_leveling.options`, exe strings around the blocked texts; only the blocked-text ids
  exist). So no unlock state is stored and none is invented. The server sends `#ru` messages on every faction level-up
  (notices.md), but the mask is all open, so the client never asks `3A` in practice; the reply is ready for when an unlock
  rule is found.

## 35 00: faction battle / reputation refresh

Sent by `refresh_faction_battle_status` (0x2001C7B90) and, always, at the end of the `#ru` message (0x1C31F9). The reply is
the `8E` with the battle status: `servers/lobby-server/src/faction_battle.rs` (`push_status`, PROVEN in progression.md
section 5). The `#ru` path reads the stored previous points (client state `L+0xEBC14 + 4 * faction`) and the fresh points
from the 8E (0x18ADE0) for `faction_reputation_delta`, which fits an 8E answer.

## 39 00: ask_new_contract_info (reply PROVEN)

* Senders: the Flash callback `ask_new_contract_info` (0x2001EA470, call 0x1EA490) and the tail of the `83 4A` sub 0
  reply (0x200191928: `37 00` then `39 00`, 0x19193E).
* Consumer: contract data are NOT a message of their own. `0x2001C56B0` (status case 10, run after every `94`) walks the
  `94` records (0xC2C bytes each); for a record of type 0x16 and a non-zero record count (client state `L+0x7A3528`) it
  calls 0x2001C5F00 (`root.set_contract_data`, `root.set_contract_exist(true)` at 0x1C5D21; no record: exist = false). The
  data come from the optional tail of `94` (`u8 present, u16 C, C x u8 step status, u32, u32, u32 len + config blob`,
  reader 0x18A3C4..0x18A498). 0x1C5F00 reads the blob as a binary config with the keys `amount` (u32), `actionType` (u8:
  0 = text `st_contract_complete_after_play_descr`, 1 = `st_contract_complete_after_win_descr`) and `dictId` (u16), and
  builds `daysData` entries `{type = status byte, amount}` (a status of 1 adds `amount` to a running total), plus
  `actionTime`, `useTime`, `service_dict_id` and two more fields from the tail words at +0xF0 / +0xF4. PROVEN: +0xF0 =
  `minutesLeft` (time left of the whole contract), +0xF4 = `timeLeft` (time to the next day's task; 0xFFFFFFFF =
  `challenge_available` true); in the contract object `actionTime` is always 0 and `useTime` false (`service_dict_id`,
  `useTime = true`, `actionTime = record+8` belong to the generic action builder 0x2001C6350). Unit of both: MINUTES (the
  movie's time manager ticks every 60 000 ms).
* Reply: a `94` (the only message that carries contract state).
* Implemented: `39 00` -> the current `94` (premium store data + the login calendar record, the same builder as at
  sign-in: `login::notices_push`), which carries the Winner's Contract record and tail while one runs (`premium::notices` ->
  `contract::notice`). With no running contract there is no record and the contract button stays hidden, the true state.
* `37 00` (the other half of the `83 4A 00` tail, 0x19192A) is outside this note.

## 34 00: after a rank or level-up message (reply OPEN, none expected)

* Senders: only `important_message_arrived`, after the two types that show a reward:
  type 9 (fields `nickname`, `level`, `reward`, `reward_type`; ends at 0x1C3700) and type 0xA (`character_leveling` table,
  `skill_level`, `reward_type`, `reward_id`, `reward_amount`, `player_experience`, `delta`; ends at 0x1C3B05). Both first
  remove the message from the client list and send the read answer `51` (0x2647F0, sends id 0x51 at 0x2648F1), then `34`.
* No Scaleform callback name belongs to it and no pending flag is set. No S->C handler reads state that only a `34` could
  trigger: the money, level, skills and inventory pushes (8B, 8C, 89) already follow the server-side change that produced the
  message, and the level-up silver is paid by the server (progression-audit.md).
* Conclusion: the client expects nothing provable. The server logs it and sends nothing (debug log line). Re-sending the
  pending messages would be wrong: the client keeps a message until its own `51`, so a re-send would duplicate entries.
  The server sends type 0xA (own level-up) and type 9 (a friend's level) messages (notices.md), so `51` and `34` occur;
  both are logged and need no answer.
  OPEN: the original server's behaviour (LIKELY a message-queue pull); needs a recording of the real server.

## Tests

`servers/lobby-server/tests/generic_requests.rs`: 39 repeats the sign-in 94 with no contract tail (no contract bought), 3A
answers one `96` (0x0F), 34 answers nothing. `tests/store_offers.rs`: 39 with a running contract returns its record and tail
(day statuses, minutes), none after the 3 days.

## Contracts in the game's history

* The "Huckster's Contract" (official news 2015-12-29 and 2016-02-15): 500 gold, then 100 gold after the first match every
  day for 30 days, one active contract at a time; a timed web-shop offer that ended 2016-02-21. It matches the client's
  `ActionProfiteer` page (daily reward, "after play / after win" task text, 30 days). It is not offered: it predates the
  0.69 era.
* The Colt M1911 "Russian Contract" (dict 1162 / 2173) was the 15-daily-logins reward (official patch notes 0.45ac), not a
  contract reward.
* "The Winner's Contract (3 days)" (official web shop, listed 2019-07-17 .. 2022-05-27): 20 000 silver for the first win
  each day for three days, 60 000 silver in total, active on purchase. That is the silver "after win" form of the client's
  `ActionProfiteer` page; it is the contract Skhid sells.
* PROVEN: the client has NO way to buy a contract by itself (the contract window's only button starts matchmaking;
  `39 00` just asks for a refresh when a timer ends); the page and the contract button next to the premium button appear
  only after a `94` record of type 0x16. Selling flow: an actions-tab card bought with `5C 01` -> `BC` sub 1, then the 94
  type 0x16 record with the day statuses and the blob (`amount`, `actionType` 1 = "Win 1 match", `dictId` 214 = silver).
  `B1` type 0x16 is the daily contract reward notice ("Received", opens the contract window).

## The Winner's Contract

The Winner's Contract (3 days) is sold for 100 gold as an actions-tab card (`actions.md`, store.md "Store offers and
pictures" for the full rules). Code: `servers/lobby-server/src/contract.rs`; state: schema v20 table
`contracts (account_id, started, won_mask)`.
* While a contract runs every `94` carries record type 0x16 and the tail: C = 3 day statuses (1 = paid, 0 = not yet or
  missed), A = minutes left of the contract (rounded up), B = 0xFFFFFFFF while today's win is open, else the minutes until the
  next day starts (on the last day: until the end), blob `{amount 20000, actionType 1, dictId 214}`.
* Days: 3 x 24 h from the purchase (CHOICE). The first won online PvP match (random, ranked, clan battle) of a day pays
  20 000 silver once and sends 8B, the `94` and `B1` 0x16 `[20000][0]`; a loss, a second win or a local / custom training /
  PvE match pays nothing. One contract at a time (a purchase while one runs is refused with `BC` sub 1 result 0).
* `39 00` (the client asks when its countdown B ends) gets the current `94`: the next day's open task, or no record after the
  end (the client then hides the button).
* LIKELY: a missed day stays 0 (the client sums only status-1 days; no other status value is drawn differently in
  `ActionProfiteer`, whose last-day check only compares the status with 1).
