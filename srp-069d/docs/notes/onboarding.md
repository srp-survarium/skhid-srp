# New-player onboarding (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

What a player sees after the first sign-in: the nickname screen ("HOW DO WE CALL YOU"), then choose a faction, then
the full lobby; the tutorial can be played or skipped. Client facts are 0.69d exe addresses.

## Names: login name and nickname are separate

The server has no e-mail / Steam / VK / Google sign-in. The login screen name exists only
for signing in.

- **Login name.** Private. It is never shown, searched or compared with nicknames.
- **Nickname.** Public, unique (case-insensitive). It is chosen once for free on the nickname screen; later changes
  need the rename voucher.

Reasons: a public login name gives away half of the credentials (password guessing, and the 5-failures lockout could be
used to lock a known player out), and a rename would change how the player signs in.

Storage (`accounts`):
- `accounts.name`: the login name.
- `accounts.nick`: the nickname. NULL = not chosen yet; the display name is then the neutral generated name
  `Player<account id>` (CHOICE: the login name is never shown to anybody, the player included;
  the account id is the low part of the public id; no source says what the original showed). The one function is the SQL
  `COALESCE(nick, 'Player' || id)` used by every store query that returns a name (`Store::account`, `find_account`,
  friends, ignores, clan members, ladders, complaints); the lobby only ever uses those names (`peer.name`, 8F, the 87 body
  name, chat, squads, rooms, results, 93, clan lists, hero / PvE lists), so there is no second place that builds one.
  A player on the nickname screen sees the neutral name in his own 8F, and so does everybody else, also after
  `skip_tutorial` in the same session.
- Uniqueness checks and the player search (4E 04) use `nick` only, so an account still on the nickname screen cannot
  be found.

## The client's step machine (PROVEN)

S->C `9D`: `u8 A, u32 F, u32 G, u8 faction, u16 H`. After the `5B 02` lobby-load handshake (`F |= 0x400`), and again
when a `9D` with `A != 0` arrives while the client is still in its loading step, the client picks a step from `F`
(0x200173470):

| F | Step |
|---|---|
| bit 1 or bit 5 set | full lobby |
| else bit 7 clear | nickname screen (type 4: `48 04`, `48 05`, `player_face_selected`, `skip_tutorial`) |
| else bit 6 clear | choose faction (type 6: `change_quest_faction` = `4A 00`) |
| else | full lobby |

`skip_tutorial` (`4A 02`) picks the faction-pick step when the 9D faction byte is 0, else the full lobby.

## Server side

`F` is stored per account (`profiles.onboarding`, schema v11).

| Bit | Meaning | Set by |
|---|---|---|
| 7 `NICKNAME_CHOSEN` | nickname screen done | the free first `48 05` |
| 6 `FACTION_CHOSEN` | faction chosen | `4A 00` (also picks the kit armour of that faction while nothing is earned) |
| 5 `TUTORIAL_DONE` | tutorial done | `4A 02` skip, or the tutorial match played to its end |
| 1 | full lobby + forced faction pick while the 9D faction byte is 0 (PROVEN, see below) | sent (not stored) when bit 5 is set, a nickname exists and no faction is chosen |

`9D` sends `F` (plus bit 10 after the handshake) and the active faction once bit 6 is set (0 before). While bit 7 is
clear, bits 1 and 5 are left out of the sent `F` (stored unchanged): `skip_tutorial` is offered on the nickname screen and
`4A 01 01` sets bit 5 too, and with bit 5 the client would open the full lobby without a nickname, showing the private
login name. Such an account gets the nickname screen at its next sign-in.

- **New account.** `F = 0`: nickname screen first. Accounts created by auto-register get the data's new-player kit
  (progression-audit.md).
- **Accounts from a v10 database (v10 -> v11).** `F = 0xE2` (full lobby) and nick = login name: they already play in
  the full lobby.

### 48 04 `request_nickname_validation` (`str name, u32 3`; the client repeats it about once a second)

Reply: `83 48 04 str name, u8 valid`, and when `valid` = 0: `u8 reason, u32 n, n x str`.

- `valid` = 1 when the name follows the account rules (1..=32 characters, no leading / trailing space, no control
  character) and no other account uses it as a nickname.
- The reason byte (PROVEN): the exe reader of `83 48 04` (0x2001920DA, call of 0x2001C7650) maps it to a localisation key and
  hands the localised TEXT to the movie as `handle_nickname_validation(valid, name, message, suggested names)`; the movie
  shows `message` under the name box (`MessageEnterName.handleNicknameValidation`). Known bytes: 14 `st_wrong_symbol_clan_string`
  ("The string contains non-printable character"), 15 `st_already_exist_clan_string` ("This name is already reserved."),
  21 `st_short_clan_string` ("The name is too short"), 22 `st_long_clan_string` ("The name is too long"), 23
  `st_space_symbol_clan_string` ("Invalid delimiters"); any other byte shows an empty warning. The server maps: taken -> 15,
  empty -> 21, over 32 characters -> 22, leading / trailing space -> 23, control character -> 14
  (`rename::refusal_reason`). The clan-name reply uses its own table (bytes 0..6).
- Suggestions (CHOICE): up to 3 free names "name1", "name2", ...
- `st_string_pattern_error` and `st_enter_name_warning` are not selected by any reason byte of this reply (the reader's
  table has only the five above).

### 48 05 `change_player_nickname` (`str name, u8 face, u32 voucher uid`)

- **On the nickname screen** the client sends uid `0xFFFFFFFF` (client log: `05 04 "test" 06 ffffffff`). While the account
  has no nickname, this is the free first choice: the nickname and the face are stored, bit 7 is set, the reply is
  `83 48 05 01, u32 0, u8 face`, then `8F` with the new name.
  - uid 0 is deliberate: the reader 0x200192C33 stores the face and skips the inventory decrement for uid 0
    (the uid is compared with 0), PROVEN.
  - The client then leaves the step by its own `tutorial_nickname_ok_clicked`.
- **Later** only a rename voucher the account owns works (rename.rs); the face byte is stored as well (the voucher
  description: "change your nickname and appearance").
- **Refused** (taken, malformed, no voucher): `83 48 05 00`.

### Face

`profiles.face` goes to the 87 account body `+0x2F6` (`AccountBody::b2f6`), so the chosen face survives a re-login.

## Open

- The 48 04 reason codes (Flash).
- 9D `G` / `H`: PROVEN (0x200177ED0, compare 0x200177FE6): `G` = battles played (the client adds 1 on every `97`,
  0x20018B4DF), `H` = battles needed before modes other than mode 4 unlock; a mode with `G < H` shows "The mode will
  be unlocked after %VALUE% battles" (`H - G`). Sent as 0 / 0 = nothing locked by battles (the original threshold is
  not in any file). Bit 1 of `F`: see "Bit 1" below.
- Whether the original server sent anything after the faction step (`48 26` is sent by the client then: the clan data
  request, answered by clans.rs).
- The tutorial match: the script (`gameplay/scripting/tutorial.lua`) spawns profile index 3 and later switches to 0,
  1 and 2 with no check (exe 0x2003CDC2E, then 0x2002F11B0 writes through the missing slot at 0x2002F11F4), so options
  with one profile crash the client when the script spawns the player. The `5B 02` options carry what the tutorial data names (`local_match::tutorial_options`, data
  `data/tutorial_069d.json`):
  * the human on team 0, first record, with the four kits `tutorial_player_0..3` (`gameplay/players/ai.profiles`) as
    profiles 0..3, all present. The PPSh kit has its magazine and collimator installed, the Mosin kit its scope.
  * the nine bots of `tutorial.options` `scripting.profiles` on team 1. Each record is named as in the script (the scene
    binds bot records by name, 0x2003C9464) and has an AI id, the face `head`, the named kit and the `ai_params` AI
    block.
  * 10 players.

  LIKELY (not yet run against the client).

## Checks in the game

Use a fresh account name (auto-register):

1. The nickname screen appears.
2. Typing a name that exists shows the refusal (note the text shown: it tells the reason code).
3. Choose a nickname and a face, then a faction; skip the tutorial.
4. Sign in again: the full lobby, the chosen nickname in the lobby and in chat, the face kept.
5. A second account cannot find the first by its login name, only by its nickname.

## Bit 1 (PROVEN)

0x2002CA880 is the callback `choose_faction_message` (slot 1 of the name table 0x20097B530; slot 0, 0x2002CA810, is
`change_quest_faction`, which sends `4A 00`). Flash never calls it: the client calls it itself from the progress status
function 0x2001C1980 when the `9D` faction byte is 0 (0x2001C1CCB; the function exits early while the `96` mask is 0). With
`F` bit 1 set it makes step 3 current, else nothing. Step 3 (start 0x2002CA3D0) opens the "Faction" subsystem in the normal
lobby with only the faction-pick handlers live, no tutorial mode, no tutorial-complete message; `change_quest_faction` leads
to step 2. Step 6 (bit 6 clear, start 0x2002C9D00) is the same pick plus `set_tutorial_mode(true)`; its finish sends `48 26`.

Server rule (CHOICE built on that mechanism): an account with a nickname and bit 5 (tutorial skipped or played) but no faction
gets bit 1 in the sent `F`. Without it the client would open the full lobby with no faction (bit 5 wins in the picker) and
nothing would ask for one. The original server's rule for bit 1 is not in any file.
