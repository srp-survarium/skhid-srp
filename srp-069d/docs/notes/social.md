# Social area: login refusals, chat channels, clans, custom match rooms

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

PROVEN = read from the exe (address) or AS3; LIKELY = inferred; CHOICE = server rule with no client evidence.
Addresses are 0.69d exe VAs (image base 0x200000000). Every reply carries real stored state; what could
not be found is left out and listed under "Open".

## 1. Login refusals (`0C`)

Layout PROVEN (0x20011F140): `0C u8 code`, plus `u8 reason, u32 value` only for codes 2 and 3. Code to text table PROVEN
(0x20010BAD0 / 0x20012A060). `login_server::Refusal` carries `{code, reason, value}`; a bad password is code 4 (code 1 is
"hardware banned").

| situation | code | where |
|---|---|---|
| wrong digest, unknown account (auto-register off), account name not registrable | 4 | `AccountAuth::decide` |
| 5 failed sign-ins of one account name within 60 s | 5 | CHOICE (`AttemptLimit`), lockout runs from the last failure, a success resets it. Per address is not possible: the `Authenticator` does not see the address |
| banned / suspended (account columns `ban_kind`, `ban_reason`, `ban_until`) | 2 / 3 + reason 0..15 + minutes left | `Store::set_ban`; value = minutes left, a permanent ban sends `u32::MAX` (unit LIKELY, 0x20012A..: client multiplies by 60) |
| already online | 7 | CHOICE refuse (`AlreadyOnline::Refuse`; `Kick` stays for tests only): the client has a dedicated text, and a duplicate sign-in is the usual cause of lobby confusion. `kick` grants the new sign-in and drops the old connection (a crashed client otherwise blocks its owner for the 10 s lobby timeout) |
| client version differs from `accounts.client_version` of the config (the sign-in request carries `0.69d0`) | 8 | only when it is set |

Admin: `srp ban <account> <ban|suspend> <reason 0..15> <minutes|0=permanent>`, `srp unban <account>` (the database of `--config`).
Schema migration v2 -> v3 (`SCHEMA_V3_UP`) adds the ban columns and the clan tables.

## 2. Chat channels (`4C`, AF)

Sender PROVEN (0x2002641E0): `4C u8 channel, TARGET, str text`; TARGET u64 for channel 4, u32 context id otherwise; id 0 / -1 means
the client does not send. The context ids are client fields set by pushes: +0xA8 clan = id of 9A sub 0 (0x18BF8C), +0xB8 squad = id of AE
(0x18DD59), +0xBC custom match = room id of 98 (0x18BA9F), +0xAC match = set when a game is joined (0x18DFFA; the 82 message of an
online match). `chat.rs` checks the context id against the server's own membership and relays to the
other members (never to the sender: the client prints its own line, 0x2643E0), skipping receivers that ignore the sender. Chat lines
carry the sender's clan tag (AF kind 5 `abbr`). Channel 2 is server only; client lines on it are dropped.
Channels 5 / 6 / 7: `Registry::set_match_context(account, context, team)` makes the relay work (5 = whole match, 6 / 7 = own team);
the online-match start calls it.

## 3. Clans

Storage `accounts::clans` (tables `clans`, `clan_ranks`, `clan_members`, `clan_invites`, `clan_reservations`), messages
`protocol::s2c::clans` (9A), logic `lobby-server/src/clans.rs`. 9A layouts PROVEN from the reader (0x18BD60); the u16 after the member list
is the receiving player's own index in the list (its use at 0x280572 finds the local rank). Limits PROVEN: 10 ranks, 107 members.
Rank mask bits (unpacked by 0x2800A3): bit 4 invite, 3 kick, 2 change rank, 1 daily message, 0 battle commander.

* `48 09` / `48 0A` -> `83 48 09|0A str, u8 ok [, u8 reason]` (PROVEN 0x19223E; reasons 0 short, 1 few letters, 2 exists, 3 space, 4 wrong symbol, 6 long, from the jump table 0x192DAC). Rules (CHOICE): name 4..32, abbreviation 2..5.
* `48 07` reserve, `48 0D` pay (CHOICE 50 000 silver, also put in 8D slot 4 = `set_clan_creation_cost`; the game data has no price), `48 08` create (needs the paid reservation). Founder = rank 0.
* Default ranks: the four standard ranks (section "Client texts in the store and clan ranks").
* `48 10` invite -> `83 48 10 u8 0, u32, u64 target, str128, 8 bytes, str128 text`; the client answers with `4D kind 7` (PROVEN 0x1926E4..0x192945), which the server turns into B1 type 7 for the target (only if the invitation is stored); the answer is `48 0B` (accept = 1, LIKELY). Pending invitations are delivered again at sign-in and expire after 3 days (CHOICE).
* `48 11` permission (index order invite, kick, change rank, daily message, battle commander = LIKELY from the five callbacks 0x2815D0..0x281790), `48 13` rank up (to rank 0 = hand over, the commander drops to rank 1), `48 12 a=0` rank down, `48 15` / `48 16` create / remove rank, `48 17` daily message (sent to members as 9A sub 1).
* Leave / disband / cancel registration: `48 0F a` with a = 1 leave, 2 disband, 0 cancel is a CHOICE (the dialog ids are not decoded); kick is `48 12 a=1` (LIKELY: the system-message variant, 0x1E8F76). The commander must hand over before leaving (CHOICE).
* Clan experience and level (`game_data::clan`, `clans::record`, called by `stats::record_match`): PROVEN only that the client
  draws a level number and a bar `minimum / maximum / value` with the tooltip "value / maximum" (`ManageClanWindow.setClanProgress`
  of clan_window.swf; the 9A `players` value is not used by it). Neither the game data (`gameplay/clans/clan_name.options` holds the
  name rules only) nor the official posts have a clan experience source or level table, and no client message announces a clan level
  (`st_clan_level_up_*` belong to the hero-of-the-day window). CHOICES: a member's finished online match (`MatchFacts.match_type`
  known: random or ranked) pays his clan 20 experience for a win, 10 for a loss, nothing for a leaver or a local training match;
  level n starts at 500 x (n - 1) x n experience (0, 1000, 3000, 6000 ...), 20 levels; the bar is 0 .. span of the level with the
  experience inside it. The stored `clans.xp` is what 9A sub 0 sends; a level or bar change is pushed to every online member.
* Every change pushes 9A sub 0 to every online member; a member going online / offline pushes 9A sub 2. 9A is part of the login pushes.
* Refusals are `84 48` with a text (client-shown).

## 4. Custom match rooms

`62 18..24`, S->C 98 (room state) and A1 (aborted, `str`). Layout of 98 PROVEN field order (0x20018B7F0), names from the UI function 0x2001C86C0
(`root.custom_match_fill_state` keys). 99 is NOT a custom match message: its block (0x20018ECDD) fills `root.fill_pve_victory_screen`. A0 has no payload.
Rooms are in memory (`rooms.rs`): create (kind 0 training, 1 clan match, needs a clan), invite (B1 type 8), answer (`62 1B`), leave (`62 1C` with the own id: the mini panel's X and the window's back button, PROVEN; host leaving closes the room), kick (the host's row button sends `62 1C` with the member's id, PROVEN CustomMatchPlayer), team choice (`62 1D`), parameters (`62 1E`: map, weather, mode, region, chat, ignore player, order of the Flash call; validated against the game data rows `matches::modes`; region = a `9E` server id), team sizes / AI slots (`62 1F` / `62 20`: team 0/1, count; bounded by half the row's max players), swap teams (`62 24`), chat channel 9. Every refusal is an `84 62` text and a log line.

**Team byte of `62 1D`** (PROVEN): the row buttons call `custom_match_change_member_status(id, 1 | 2 | 3)` for A / B / ?, and the
exe builder 0x2002823B0 sends the value minus 1 (0x2002823DC), so the wire byte is the role itself: 0 A, 1 B, 2 unassigned.

**Leaving = `A0` / `A1`, never an empty 98** (PROVEN): `A0` (no payload, `root.custom_match_ended`) and `A1` (`str` reason,
`root.custom_match_aborted`, shown in a message box) both rebuild the client's 0x3208-byte room record empty (0x20018EF4F / 0x20018EEB7) and
hide the room window and the lobby mini panel (`setData(null)`, `onHideCustomMatchWindow(null)`). A 98 with room id 0 is handled like any
room (0x2001C86C0 has no room-id test): the panel then shows an empty room, "0/0" per team, and stays. So: the leaver gets `A0`; when the
host leaves the room closes and the others get `A1 st_custom_match_aborted_by_commander`; a removed member gets `A1
st_custom_match_kicked_from_match`; a `62 1C` from a client the server holds in no room is answered with `A0` (it clears a stale panel).
These reasons, `st_custom_match_match_started_without_you` and the other `st_custom_match_*` / `st_cant_*_custom_match*` keys are
referenced nowhere in the exe: they are texts the server sends.

**Play button** (0x2001E8070, PROVEN): with a room (record +0x3018 != 0) the client looks up its own member (+0xA0 id). Not the commander
(+0xB7): `62 22` with the toggled ready. Commander: `62 23` only when every non-commander member on team A or B (+0xB6 0 / 1) is ready
(+0xB8; unassigned members are not checked) and the 98 `tokens allocated` >= `tokens required`; it then waits for `83 62` / `84 62`.
The members' slots: the window counts AI slots as bot rows of a team (`bot_tag`, id 0) and a team is full when humans + bots = AI slots +
player slots, so `player slots` bounds the humans.
`ready` in 98 is the member's own `62 22` toggle (PROVEN, see `clan-battles.md`; the host counts as ready). 98 `team` is the client role: 0 A, 1 B, 2 unassigned (PROVEN).
**Start (`62 23`)** (`online.md`): the host's Start creates an online match on the room's row (map, mode, variation) and sends
`82` to the team members when online matches are on and the match server serves the row's project (with a world host: the projects of
its config). Rules: the host only; each team has a human or a bot and at least one human plays; humans within the player slots; humans +
bots within the row's team size; the team members (not the reserve) ready; tokens complete (one per team member without premium; the
reserve needs none, CHOICE). A row the match server does not serve is refused with `st_locked_by_server` ("The mode is unavailable");
the other refusals with the `st_custom_match_error_*` texts or `st_cant_start_custom_match_error`. The room then closes: the players get
`A0` after their `82`, the reserve `A1 st_custom_match_match_started_without_you` (LIKELY: the text exists only for this).
**Bots in rooms**: training rooms only (the client hides the bot settings of a clan room and whenever `max_ai_count` is 0, PROVEN
CustomMatchWindow.setData). The server offers them (`max_ai_count`) when the match server's world host drives bots, at most 6 per team
(`online::BOT_TEAM_MAX`, the bot-filled match limit) and within the row's team size; the Start passes the room's counts per team as the
match's AI players (`online::start_room`, the queue's bot path; their level = the humans' average equipment level).
**Bots ignore players**: the room byte (98 `ai_ignore_players`, `62 1E` last byte) is the client's "harmless bots". In 0.69d
it is no match-options field: the exe keeps it in the plain global byte 0x20F43FE53 (the bots tab's Play writes it from `ignore_player`
at 0x2001E8689, the shooting range sets it at 0x200133817, `close_current_match` clears it at 0x2001F520A). Its AI readers (PROVEN): the
vision target filter 0x20032CA70 (used by the vision sensor's `query_target_players`, vtable 0x20098EA48 slot 0 0x20032D090 -> 0x20032CAC0, and
by `select_enemy` 0x20036C320 at 0x20036C4B8, which then drops every human from its candidate list) and the sound filter 0x200368B50 (true for a record with `ai_id == -1`, from the sound
fan-outs 0x2002A2AA0 / 0x2002D74E0 / 0x20038A770 and `select_enemy`): set, the bots neither see nor hear the human players. Its other
readers (unlimited ammo, grenades, placeables, medkits) also need `single_player`, which online options never set, so the host's world
stays the clients'. The server passes it: `BotFill::ignore_players` -> `MatchSpec.ai_ignore_players` -> world host protocol 7
`Create.ai_ignore_players` -> the host child writes 0x20F43FE53 = 1 before its AI ticks (log line "the AI ignores the human players").
Queue bot fills never set it.
**Room lifetime after Start**: the client is built to keep its room through a match: its room record (`L+0x79C528`, room id
`+0x79F540`) is reset only by its constructor, a new sign-in (0x2001853E0, from `sign_in_on_message_received`), `A0`, `A1` and the
client's teardown; the `82`, the level load, `close_current_match` and the `97` do not touch it. The lobby movie's `start_loading`
leaves the PvE window alone when the custom match window or its mini panel is visible (it expects them visible at a match load), and
`custom_match_fill_state` only re-opens the window for a room it is not already in (`isInRoom(room_id)`). The exe never re-pushes the
room to Flash by itself (0x2001C86C0 has the 98 reader as its only caller): a room kept on the server would need a 98 after the match.
Choice: the server closes the room at the Start (each match is paid with tokens again anyway); what must work is the way back into the
running match: a member who drops or whose game crashes keeps his seat and gets the seat's `82` again (at the drop and after a new
sign-in), exactly as in a queue match, and the closed room sends him nothing (no 98 / A0 / A1) while his seat is kept; a new room is
refused then (`st_custom_match_error_player_already_in_match`, he is in a match).

## 5. Complaints and compliments (`48 06`, `comment_about_user`)

* Request (PROVEN, builder 0x18D870, callback 0x1E8DA9): `48 06 | u64 player | u8 kind | u8 a | u32 b`. The three small fields are
  Flash arguments of the call (kind and `a` from argument 1 / 2, `b` from argument 3, or the client's own `[state + 0x1074]` when
  the call has only three arguments). The Flash that makes the call is not in the extracted movies: what `a` / `b` hold is OPEN, they
  are stored as sent.
* Answer (PROVEN, reader 0x19253E): `83 48 06 <u8>`: 1 shows `st_claim_added` ("Complaint sent successfully."), anything else
  `st_compliment_added` ("Praise sent successfully."). LIKELY the request's kind byte is that same value (1 = complaint).
* Limits (texts PROVEN in the localisation, numbers OPEN): "You can't file a complaint against a user twice a day!"
  (`st_claims_count_per_user_owerflow`), "You've filed the maximum number of complaints for one day."
  (`st_claims_count_per_day_owerflow`), "You've used the maximum number of praises for one day."
  (`st_compliment_count_per_day_owerflow`). No S->C reader takes a refusal code for them (the `48 06` answer has only the one
  byte), so how the client shows them is OPEN.
* Implemented (`lobby-server/src/comments.rs`, `accounts/src/comments.rs`, schema **v16** table `user_comments`): every
  accepted comment is stored (time, sender, target, kind, the two fields). Rules: no comment about yourself or about a name that is
  not an account; one complaint per target per UTC day (PROVEN by the text; the UTC day is a CHOICE); the daily maxima are the
  settings `social.claims_per_day` / `social.compliments_per_day` (0 = no maximum, because the number is OPEN). A refused comment gets
  no answer (the client has none to give) and is logged.
* Moderator: `srp comments [<account> [<limit>]]` lists the newest comments (kind, sender, target, the fields); with an account only
  those about it. No player-visible effect: no 0.69d file or text mentions karma or an automatic penalty for complaints (searched the
  strings, the localisation and the web notes).

## Open (not implemented, nothing faked)

* 93 player details (`protocol::s2c::details`, `lobby-server/src/details.rs`, implemented). PROVEN from the exe: the reader 0x20018B550 reads a statistics block (layout in the module docs; 0x182350, tail verified at 0x182AAC..0x182AFE:
  `u8 n, n x {u32,u16,u8,u8 k,k x u8}, u8, u16, u8, u8`) and then the account body; only the body is handed to 0x1431C0 -> Flash `root.fill_victory_screen_player` (name, level, state, item_slots, artifact_slots).
  The statistics object is only destroyed afterwards (0x20018B609..0x20018B62C), so the client never uses its values. The server therefore needs no combat or match counters and no schema step: the block is written with every value 0 (mask 0, no lists) and the
  body is the real stored account body of the target. Answered for the requester's own id and squad mates (CHOICE); bots / other ids get no answer. Check in the game: click your own row on the results screen: name, level and equipment appear.
* `62 19` join request (no caller). Custom match Start, tokens and the ready toggle: section 4.
* Clan experience: CHOICE rules above; its input is `MatchFacts.match_type` of an online match. Clan rewards: no data, no client
  window for them: not implemented.
* Foreign clan view (9A sub 0 with `own != 1`, member tail of `[79C520] == 0`, reader 0x20018BF5F..0x20018C184): NO client request asks
  for it (the 198 Scaleform callbacks and the `48 xx` builders have no clan-view / clan-search call; the clan window callbacks
  `clan_window_opened` / `clan_members_tab_opened` use 0x281F10; `48 26`, LIKELY the clan data request, has no clan id) and a pushed non-own 9A would REPLACE the player's own
  clan state in the window. So there is nothing to answer and nothing is built; clan search / list has no request either.
  PROVEN: no Flash-to-engine call or clan string asks for another clan
  (`root.set_league_clan` is the own clan's league ladder; the only player card request is `victory_screen_request_player_data`);
  the non-own 9A form is the clan being CREATED (members with a status byte, tail `u8 paid, u8, u32, u8 status`: status < 3 =
  name / abbreviation invalid, window state 0; >= 3 state 1; paid = state 2; 0x20018C2C8..0x20018C40B).
* Clan battles: everything (evidence, rules, results, what stays open) is in `docs/notes/clan-battles.md`.
* 4F is not needed by the server: context ids come from the server's own state.

## Custom match results

Dev notes of 0.69c: achievements can no longer be earned in custom matches. Rule for the server: custom match results must not count for achievements.

### Complaints and compliments: the exe side

* `48 06` = `comment_about_user(id, index, subIndex, match_id)`: `kind` = index (1 complaint, 2 compliment), `a` = reason (complaint
  1 bot / cheater, 2 swearing, 3 team kill, 4 account trading; compliment 1 friendly, 2 great player, 3 born commander), `b` = match id
  (0 from the lobby chat). PROVEN. The client has no daily maximum.
* Refusals use `84 x 48 01 <key>` (PROVEN text channel, localised at 0x200176430): `st_claims_count_per_user_owerflow`,
  `st_compliment_count_per_user_owerflow`, `st_claims_count_per_day_owerflow`, `st_compliment_count_per_day_owerflow`,
  `st_error_during_storing_user_comment`. The keys exist only in the localisation (LIKELY the original server's). The
  compliment-per-user text states a once-a-day rule for compliments too: applied.

## Localized texts

`AF` chat lines are not localized (reader 0x200264A40, printer 0x200159A80), so the server sends none of its own:
no welcome line, a private line to an offline player gets `84 4C`
`st_friend_state_offline` ("Offline"), a friend request shows only in the receiver's friend list (state 4). LIKELY better:
`B1` type 0 with the sender makes the client's own `st_add_to_friends_request` popup (0x2001C2A86); its accept / decline
reply is not traced (OPEN), so it is not sent.

## Client texts in the store and clan ranks

* The exe never localizes a rank name; the clan window does (PROVEN): a rank sent with the wire `default` = 1 is shown as
  `LocalizationData.getName("clan_rank_" + name)` upper-cased (`ManageClanWindow.setRanksData`; member rows through the
  member's `default_rank` flag, `ClanListRenderer`), else the name as sent; a failed lookup shows `#` + key. The lobby text
  table has exactly `clan_rank_commander` / `_assistant` / `_warlord` / `_soldier` = "Commander" / "Deputy commander" /
  "Sergeant" / "Soldier" (all 11 languages). The client disables Delete for `default` = 1 and has no rename request.
* New clans get those four standard ranks (lowercase names, wire `default` = 1; permissions CHOICE:
  commander all, assistant invite / kick / daily / battle, warlord invite / battle, soldier none), new members join at
  soldier (CHOICE, server side only), a standard rank cannot be removed (`st_cant_delete_default_rank`), a member row's
  `default_rank` = its rank is standard. Schema v21 turns the default rank rows of existing clans into standard ones
  (commander / assistant / soldier). OPEN: the original permissions and join rank; to check in the game: the clan window.

## B1 field order, invite answers, friend requests

* `B1` layout (PROVEN: reader 0x200265550 stores the first byte at record +0xB0 and the byte after the clan tag at
  +0xB8; the consumer call 0x2002658B0 passes +0xB8 as the TYPE; the live-tested 0.26g encoder writes the sender type
  first and the kind after the tag): `u8 flag (unread, 0), u64 sender, str128 name, str32 abbr, u8 TYPE, str256 text,
  u32 id`. With the two bytes swapped every B1 (level-up, friend level, faction rank, clan / custom / squad invites,
  contract reward, premium bought) would reach the client as type 0, a friend-request row.
* Invite answers: 0 = ACCEPT, 1 = decline (PROVEN: `NotificationInvite` / `CustomMatchInviteMessage` / `FriendsMenu` send
  0 for accept; `important_message_read` 0x2001E88E0 passes it unchanged: clan `48 0B` via 0x20018CDB0, custom match `62
  1B` via 0x20018D2A0).
* Friend requests: `4D 0` that leaves a request -> the online receiver gets `B1` type 0 {sender, nickname, clan
  tag, empty text, id 0x4000_0000 | requester account}; the friends window lists it under REQUESTS with the client's own
  "Friend request" and Add / Cancel (the Friends button counts and blinks; not a popup). The answer is only `51 id answer`
  (0 = Add -> friends, 1 = Cancel -> removed); both lists are pushed. Unanswered requests are sent again at sign-in; an
  ignored requester's are not sent. B0 05 / 07 carry no status-4 rows (they would show each request twice, with
  dead buttons). Type 2 is the squad invite; no "request accepted" notice type exists (the requester sees the new B0 05).
* OPEN: the original server's first B1 byte; whether `GetUInt(undefined)` = 0 for status-4 rows; the friend-limit texts
  (`st_friend_limit_reached*`, LIKELY `84 4E`). To check in the game: a level-up notice and a friend request.

## Protocol audit

See `protocol-audit.md`: 48 0F / 0E / 14 / 13 / 12 01 clan requests, the 9A member level, 62 21 token deltas.

## Clan registration

After `48 07` (name reserved) the server sends `83 48 07` and the 9A sub 0 REGISTRATION form (byte after the tag = 0):
members `{u64 id, str128 name, u8 rank, u8 status (0 = ready)}`, then own index, commander, P (supporters fine; sticky,
window state 2 = payment), Q (paid; sticky), `u32 collected silver` (only grows), S (3 = names fine). The form is also
sent at sign-in while a reservation exists and after every deposit. `48 0D u32 amount` = a partial deposit: taken =
min(amount, cost - collected), silver only (8D slot 4, 50 000); the "Register" button is enabled by P and Q
(`set_clan_creation_enabled`), and `48 08` creates the clan. The founder registers alone (no supporters required:
P = 1, 9C counter 0). Never send 9A sub 2 / sub 3 to a clan in registration (sub 2 reads the registered layout).
`48 0C u64 player` -> `83 48 0C u64, str128 name` ("Player <name> successfully invited to clan.").
OPEN: member status values beyond 0; the original supporter minimum (text: at least 5); refunds when a supporter leaves
(texts: lost); the original 48 0C side effects.
