# Squads of two

The rules the code and the tests follow (the owner's of 2026-10-31: "we should return the ability
to start in squads of 2 (not 3)"; "should be seen in room UI too"). Code: `session::squads`,
`session::v026g::rooms` (`join_with`, `next_ready_in_queue`), the lobby's `rooms.rs`
(`room_mate`, `room_quick`, `room_join`, `room_late_join`, `squad_ready`) and `social.rs`; the
movie's `SquadList`, `RoomsLook.squadTag`. Tests: `core_tests::rooms_tests::squads`, the session's
`squads` and `rooms` tests, `scripts/rooms-start/check.sh` (`checkSquads`).

## History

- Squads of three were played until 2026-10-13, when `SRP_SQUADS=off` went live (#38): three
  strong players on one side were what kept the sides from being even (57 of 106 matches).
- #171 (squads of two, `SRP_SQUAD_SIZE`) was merged and reverted the same evening (#172,
  2026-10-17): rooms replaced the queue that day. #173 (squads of two with rooms) and its port in
  #252 were closed on 2026-10-21, parked.
- Until this note the server kept squads of three, with the rooms drawing a squad on one side,
  but live had them off, Play took nobody along, and a squad's commander could not press Play
  (the game's Play gating waits for a "ready" mate, which nothing set any more).

## Rules

- **Size:** two at most (`MAX_SQUAD_MEMBERS`), for every lobby. A squad of one is none.
- **Invite:** once a squad has two, nothing invites a third: the movie's squad panel has two
  places, and neither the friends' nor the chat's menu offers "Invite to squad" (the game's own
  `isSquadFull`, at 2); a player search left open invites nobody. The server sends no invite
  from a full squad and adds nobody to it; of two invites accepted at once only the first is
  added. Silent, as every squad refusal (the game has no text for one, `social.md` §9.2).
- **Who comes along:** the mate free in the lobby: its game there and told its room, not in a
  match or on its way to one, not in a clan team, its kit able to play. A mate in a room comes
  along with a Join (it leaves that room); with Play only one in no room comes (Play of one
  whose mate is in a room goes to that room, below).
- **Play** (either member; the game lets the commander press it while it is free, the soldier
  while its commander is not, see Ready):
  - its mate in a room: into that room, beside it; that room full: its queue. A room that does
    not take it (a PIN, its max level, turned out): Play as for anyone.
  - else, with its mate along: an open room of the pair's bracket with two places, the bracket
    of the higher gear of the two (so the max level lets both in); none: the bracket's queue,
    both; none at all: a new room, both in it.
- **Join** (either member, the list, a friend's menu, Switch): both come, or nobody moves. The
  mate the room does not let in (its max level, turned out) stays where it is and the other
  goes alone. Too few places (below) with both in no room, or none on its mate's team with
  the mate in the room: it waits in its queue (both; the answer is the queue's place, the
  PIN given counting for both); one of them in another room: "That room is full."
- **Two places means two on one side** (the owner's of 2026-10-31):
  - an open room has no teams before its match: two free places take the pair, and the sides
    drawn at the start (`rooms::sides`, every way tried) always put it on one, as two places
    of 2 a side, any players, can always be split so;
  - a private room's teams are its owner's: the pair needs two places on one team (it goes on
    the team with fewer, which has them if either does), one whose mate is in needs one on its
    mate's team; one place on each team is none (`Room::has_places`), in a join as in the
    queue;
  - a match being played: below (Late join).
- **Queue:** both wait in it (the mate in no room). A pair goes in at two free places on one side, together;
  one alone behind it may take a single place, the pair keeping its places. Leaving the queue
  takes both out. A mate who may not come when the places are there (kit, gone) leaves the
  other to go in alone.
- **The list:** a mate is listed right after the other; in a private room it comes on the
  other's team while that has a place (its owner may still move it). Its owner's
  arrangement plays as it is (`rooms.md`).
- **Making a private room** takes only its maker: the mate joins with the PIN.
- **Kits and loadouts:** each member's own kit and gear (no leader's choice): each must be able
  to play and be within the room's max level, as anyone. A Join's loadout offer (`presets.md`)
  is the asker's own; a pair's Play that would queue offers no room of the other bracket (a
  loadout changes the asker's gear, not its mate's).
- **Sides:** drawn at the start with the squad on one side (`rooms::sides`, as before), one
  squad in the match (its id; the Tab list's icon). Its second member late (below) plays in no
  match squad.
- **Late join:** a member whose mate is in the match goes in on its mate's side, or waits for
  the next match while that side is full. Two who wait together go in together, both places
  reserved at once at the match server (`append_players`, `join_running_group`), on a side
  with two places, or neither (one place on each side is none): they wait in the room, told
  "The match is full.". A side's places count the places kept for players who may come back
  (`rooms.md`, a side's places in a match being played), for anyone, alone or not.
- **Leaving:** Leave takes only the one who leaves; its mate stays. A game gone keeps its
  place about 90 s as anyone's. Leaving the squad moves nobody.
- **Ready** (`squad_ready`, the squad info's flag, `social.md` §2.2): a member is ready when
  Play could take it: free in the lobby, in no room and no queue, its kit able to play. The
  game lets the commander press Play while one of the two is ready (itself, when free), and
  the soldier while its commander is not: a free commander leads the pair; a commander in a
  room, a match or away leaves the soldier its own Play (into the commander's room). The
  panel says "Ready" / "Not ready" by the same.
- **Marked in the rooms' lists:** each pair whose two members are both in a room is numbered
  1, 2, ... in the room's order, and each of the two shows a small tag with that number after
  its name, in the room's view and the list's side panel: ours in the green the game's results
  give our squad (0xB3EA97), another's in the tan of another squad (0x9D7E57). Only digits are
  drawn (the font has them), so every language fits; the name keeps its place and is cut a
  tag's width earlier (no layout jump). Wire: the state's `S;<marks>` row, the members list's
  `squad` field (`lobby.md` §2.16.3).
- **Release:** the live `.env` has `SRP_SQUADS=off`; squads are back with `SRP_SQUADS=on`
  (or the line removed) and the lobby movie of this change.
