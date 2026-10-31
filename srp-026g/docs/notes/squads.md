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
- **Only the commander acts for the squad** (the owner's of 2026-10-31: "only one player
  should be able to queue in squads"; "others have greyed out PLAY button and they cannot enter
  rooms"): its Play, Join (the list, a friend's menu, a PIN), Queue and Switch take the soldier
  along. The soldier goes into no room on its own: its Play is greyed all the time while in the
  squad, and Join, Queue, a friend's "Join room", a PIN join and New room are greyed in the
  movie (`LocalPlayerProfile.squadSoldier`: in a squad, not its leader; the greyed buttons' tip
  says "Your squad leader chooses the room."); the server refuses them too (code 17, the same
  text), "Switch and enter" before its loadout changes, and offers it no Switch. Leaving its
  room, its queue and the squad stay its own.
- **Who comes along** (one rule, `room_mate`, asked by Play, Join, Queue, the queue's turn, the
  scale-out and the late join): the commander's mate while it is free: its game there and told
  its room, not in a match or on its way to one, not in a clan team, its kit able to play, and
  waiting in no queue of its own (one it queued in before the squad keeps it: the review of
  #884). A mate idle in a room comes along with the commander's Join or Play (it leaves that
  room).
- **Play** (the commander's):
  - its mate in a room: into that room, beside it; that room full: its queue. A room that does
    not take it (a PIN, its max level, turned out): Play as for anyone.
  - else, with its mate along: an open room of the pair's bracket with two places, the bracket
    of the higher gear of the two (so the max level lets both in); none: the bracket's queue,
    the pair as one entry; none at all: a new room, both in it.
- **Join** (the commander's: the list, a friend's menu, a PIN, Switch): both come, or nobody
  moves. The mate the room does not let in (its max level, turned out) stays where it is and
  the commander goes alone. Too few places (below) with the commander in no room: it waits in
  the room's queue, with its mate (the answer is the queue's place, its PIN counting for both).
- **Two places means two on one side** (the owner's of 2026-10-31):
  - an open room has no teams before its match: two free places take the pair, and the sides
    drawn at the start (`rooms::sides`, every way tried) always put it on one, as two places
    of 2 a side, any players, can always be split so;
  - a private room's teams are its owner's: the pair needs two places on one team (it goes on
    the team with fewer, which has them if either does), one whose mate is in needs one on its
    mate's team; one place on each team is none (`Room::has_places`), in a join as in the
    queue;
  - a match being played: below (Late join).
- **Queue:** a pair is one entry, its commander's place (`rooms::Waiting::Pair`): taken and let
  in as one, at two free places on one side. One alone behind it may take a single place, the
  pair keeping its place (whether a pair may wait on so: the owner's decision is pending, a
  test kept aside). At the pair's turn, a mate that may not come then (in a match, its gear over
  the max level, its kit unable to play, gone) leaves its commander to go in alone; the mate
  stays where it is, out of the queue. A mate idle in another room waits with its commander
  and leaves that room at their turn, as a Join with places takes it (the review of 1314956a:
  the same rule, `room_mate`, for a Join, the queue's turn and the scale-out). The commander
  leaving the queue takes the pair out; the soldier leaving it leaves the place to its
  commander, alone. A pair whose squad breaks, or that another commands, waits as two entries
  at its place, the commander's first, then its mate's: nobody loses its place, and each is its
  own (or its new squad's) from then on. A soldier cannot queue, so it
  never jumps those between it and its commander (the review of #884).
- **The list:** a mate is listed right after the other; in a private room it comes on the
  other's team while that has a place (its owner may still move it). Its owner's
  arrangement plays as it is (`rooms.md`).
- **A private room** (the owner's of 2026-10-31): the commander's entry carries the pair, no
  PIN for the mate: a room it makes (a private one, or one Play or the scale-out makes) takes
  its free mate in with it (`Rooms::bring_mate`), and a join with the PIN brings it too, on
  the commander's team. Too few places: the usual pair rule (its queue, one entry, on the
  commander's PIN). A soldier makes no room.
- **Kits and loadouts:** each member's own kit and gear (no leader's choice): each must be able
  to play and be within the room's max level, as anyone. A Join's loadout offer (`presets.md`)
  is the asker's own; a pair's Play that would queue offers no room of the other bracket (a
  loadout changes the asker's gear, not its mate's).
- **Sides:** drawn at the start with the squad on one side (`rooms::sides`, as before), one
  squad in the match (its id; the Tab list's icon). Its second member late (below) plays in no
  match squad.
- **A pair alone plays no match** (the owner's of 2026-10-31): an open room whose only members
  who can play are one squad counts nobody for its start and waits as usual (no vote, no new
  text) until a third can play; then the pair is on one side. A private room plays its
  owner's arrangement, as before.
- **Late join:** a member whose mate is in the match goes in on its mate's side, or waits for
  the next match while that side is full. A soldier whose commander waits in the room goes in
  by its commander's turn. Two who wait together go in together, both places
  reserved at once at the match server (`append_players`, `join_running_group`), on a side
  with two places, or neither (one place on each side is none): they wait in the room, told
  "The match is full.". A side's places count the places kept for players who may come back
  (`rooms.md`, a side's places in a match being played), for anyone, alone or not.
- **Leaving:** Leave takes only the one who leaves; its mate stays. A game gone keeps its
  place about 90 s as anyone's. Leaving the squad moves nobody.
- **Ready** (`squad_ready`, the squad info's flag, `social.md` §2.2): the commander always: the
  game lets a commander press Play while one of the two is ready (itself), and greys a soldier's
  Play while its commander is ready, so the soldier's Play is greyed all the time. The soldier is
  ready while the commander's actions take it (`room_mate`); the panel says "Ready" / "Not
  ready" by that.
- **Marked in the rooms' lists:** each pair whose two members are both in a room is numbered
  1, 2, ... in the room's order, and each of the two shows a small tag with that number after
  its name, in the room's view and the list's side panel: ours in the green the game's results
  give our squad (0xB3EA97), another's in the tan of another squad (0x9D7E57). Only digits are
  drawn (the font has them), so every language fits; the name keeps its place and is cut a
  tag's width earlier (no layout jump). Wire: the state's `S;<marks>` row, the members list's
  `squad` field (`lobby.md` §2.16.3).
- **The scale-out** (six waiting Play users open a room, a pair counting two): the queue's
  order, a pair together or not at all: at its cut both stay queued.
- **Known, not done** (the review of #884, for later):
  - in a private room a pair waiting to go in late goes in together, on the team its owner
    put the first on, even when its owner split the two;
  - Play's loadout offer is for the room the asker alone would take; a pair's Play may go to
    another room (its bracket the higher gear of the two).
- **Release:** the live `.env` has `SRP_SQUADS=off`; squads are back with `SRP_SQUADS=on`
  (or the line removed) and the lobby movie of this change.
