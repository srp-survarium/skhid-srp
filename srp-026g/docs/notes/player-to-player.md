# What players' games send each other, and how it is checked

In a 0.26g match the games never talk to each other directly. Every byte that one player's game
makes and another player's game reads goes through the match server, and every such byte can be
forged: a modified client can send anything. This note lists everything that travels from one
player to another, shows each message with an example, and says what checks it, where, and why
that is enough for the receiving game not to crash.

The server's own messages (the match options 0xE4, entered and left 0xE9/0xEA, clock, pings) are
made by the server from its own state and are trusted. They are named here only where a player's
data ends up inside them.

Contents:

1. The picture
2. Inputs (0xDC, 0xDD, relayed as 0xEB, 0xEC)
3. Weapon pickups inside an input
4. Kit choices (0xDE, relayed as 0xF3)
5. Worlds (0xE0 chunks, sent on as 0xE8)
6. What a world contains, and how each part is read
7. When something is refused
8. The lobby: names, chat, clans
9. What is not checked yet

Related notes: `match.md` (every match message and its layout), `packet-validation.md` (the
checks' history and evidence), `snapshot_rules.md` (the world's sections), `client_patch.md`
(the patches named here).

## 1. The picture

```
  player A's game                 match server                  player B's game
  ---------------                 ------------                  ---------------

  input 0xDC / 0xDD  ------->  [S1] length, rotation, actions
                               [S2] a pickup names a place
                               [S3] on time? keep / move / drop
                               writes A's place itself  ---->  [C1] length, place < 20,
                               (relay 0xEB / 0xEC)                  rotation, actions
                                                                -> the game takes the input

  kit choice 0xDE    ------->  [S3] on time? keep / move / drop
                               writes A's place itself  ---->  [C1] length, place < 20
                               (relay 0xF3)                    [C2] the game itself: kit < 3
                                                                    and that kit has a player

  world, in 0xE0     ------->  [S4] chunks: offsets, size
  chunks                       [S5] asked for? near the clock?
                               [S6] the world check (typed views, section 6)
                               [S7] its CRC matches the other games' hashes
                               sent on as 0xE8  ------------>  [C3] envelope, unpacked size
                                                               [C4] the same world check
                                                               [C5] the game's own guards
                                                                -> the game loads it
```

`S` checks run on the server, `C` checks in the receiving game (client patches 114, 87 and 115).
Most checks are made twice, once on each side: the server stops a bad message before anyone gets
it, and the game would still refuse it if a server ever let one through.

Two properties hold for every relay:

- **The sender's place is the server's, not the message's.** A game cannot claim to be someone
  else: the server writes the place of the connection the message came from into every relay.
- **The receiving game checks before its own code reads.** The game's original readers have
  almost no bounds of their own. A forged value that reaches them can index outside an array, call
  through a null pointer or run off the stack. So every check sits in front of a reader, and
  refuses the message instead of letting the reader run.

## 2. Inputs (0xDC, 0xDD, relayed as 0xEB, 0xEC)

Every 50 ms tick a game sends its player's input: how far the mouse turned, and which actions are
held. That is most of the traffic of a match.

```
 0xDC input                           0xEB input relay (to every other game)
 +------+----------+----------+       +------+--------+----------+----------+
 | tick | rotation | actions  |       | tick | player | rotation | actions  |
 | u8   | 2 x half | u32      |       | u8   | u8     | 2 x half | u32      |
 +------+----------+----------+       +------+--------+----------+----------+
   1        4          4                 1       1         4          4
```

0xDD and 0xEC add one byte after the tick, the milliseconds into it (an input made between two
ticks). The rotation is two IEEE half floats, dx and dy, which every game adds to the player's
angles. The actions are a bit mask: move, crouch, fire, use, the quick-use slots and so on.

Example. Player 3 turns right by 1.0 and down by 0.5 while holding the action bits 0 and 5, at tick
42:

```
 sent by player 3:   DC  2A  00 3C  00 B8  21 00 00 00
                         |   |      |      +----------- actions 0x00000021
                         |   |      +------------------ dy = half 0xB800 = -0.5
                         |   +------------------------- dx = half 0x3C00 = 1.0
                         +----------------------------- tick 42

 relayed to others:  EB  2A  03  00 3C  00 B8  21 00 00 00
                             +----------------------------- player 3, written by the server
```

What is checked:

- **[S1] and [C1], the shape.** The message is exactly its length. The rotation must be two
  numbers, each under 1024: a NaN, an infinity or a huge half would turn the player to a NaN angle
  in every game (and two merged inputs add their halves, so the bound keeps the sum a number). Bit
  30 of the actions is used by nothing and must be clear. Bit 29 is the patches' marker (section 3)
  and bit 31 a flag of the game's own; both are allowed. On the relay, the player byte must be a
  place under 20, since the game stores it as an index into its 20 places without a bound.
  (`srp_validate::messages::input` and `check`, on both sides.)
- **[S3], the time.** An input for a tick the other games have already played past cannot be
  relayed as it is. The server keeps it, moves it to the next open tick and tells its owner so
  (0xEF/0xF0), or drops it and tells its owner (0xF1/0xF2), so that every game, the sender's
  included, ends up with the same list of inputs.

Refused example. The same input with dx = half 0x7C00 (infinity):

```
 DC  2A  00 7C  00 B8  21 00 00 00
         +---+-- dx = +infinity: refused, "rotation not sane"
```

The server does not relay it and tells player 3's game to drop it (0xF1), so the sender's own
simulation stays the same as everyone else's. One bad input is a strike; eight distinct bad inputs
within ten seconds remove that player from the match (`game/input_safety.rs`). Resends of the same
input count once, and an isolated bad input expires.

## 3. Weapon pickups inside an input

Picking up a dropped gun (client patch 85) has no message of its own: it rides in the input's
actions. Bit 29 marks it, and the six quick-use bits 14..19 hold a pattern that names the place
whose dropped gun is wanted. The game never sets those quick-use bits in that combination, so
the pattern cannot be mistaken for a real press.

```
 actions:  bit 31  30  29  ...  21   19..14   ...  0
               |    |   |        |      |
               |    |   +- pickup marker (patch 78/85)
               |    +----- unused: must be 0
               +---------- the game's own flag
                                 |      +------ pattern = the target place
                                 +------------- Use: cleared in a pickup
```

What is checked: **[S2]** a pickup must name a place of this match (`weapon_pickup::sane`), and
the client builds no other. Whether the gun is really lying there and free is decided by every
game alike from its own world (`weapon_pickup::decide`), so a forged pickup of a gun that is not
there does nothing in any game.

## 4. Kit choices (0xDE, relayed as 0xF3)

On the death screen a player picks the kit for the next life.

```
 0xDE kit choice          0xF3 relay
 +------+-------+         +------+--------+-------+
 | tick | value |         | tick | player | value |
 +------+-------+         +------+--------+-------+

 sent by player 3:   DE  2A  01          kit 1 at tick 42
 relayed:            F3  2A  03  01      player 3 (written by the server)
```

The server checks the time as for an input ([S3]: keep, move with 0xF4, or drop with 0xF5). It
does not check the value, and it does not need to: **[C2]** the game itself takes a choice only
if the value is under 3 and that kit has a player object (the place's tick, 0x321790). Any other
byte is ignored by every game alike, so a forged value can neither crash a game nor make games
disagree. The relay's player byte is checked as an input's ([C1]).

## 5. Worlds (0xE0 chunks, sent on as 0xE8)

A game that joins late, or that fell out of step, needs the whole world. The 0.26g server cannot
simulate, so it asks a game to upload its world (0xF6) and sends that world on to the game that
needs it (0xE8). A patched game also uploads its initial world when the match starts. A world is
up to 0xF000 bytes and is the one thing a player sends whose contents another game loads whole,
so it gets by far the most checking.

The upload is cut into chunks, each alone in its datagram and resent until acknowledged:

```
 0xE0 chunk
 +-----------+--------+-------+--------------------------------+
 | item_time | offset | total | data: 480 bytes (less at end)  |
 | u32       | u16    | u16   |                                |
 +-----------+--------+-------+--------------------------------+

 example: the second chunk of a 5,996-byte world of tick time 15,000 ms
   E0  98 3A 00 00  E0 01  6C 17  <480 bytes>
       |            |      +----- total  0x176C = 5,996
       |            +------------ offset 0x01E0 = 480
       +------------------------- time   0x3A98 = 15,000 ms
```

What the server checks, in order:

- **[S4] the chunks.** Each offset is a multiple of 480 and below the total, the total is 1 to
  0xF000, each chunk is exactly its size. A world is whole only when every chunk of it is there.
- **[S5] the upload is wanted.** After the initial world, the server takes only an upload it asked
  for, of a tick near the match clock. An unasked world, or one of a time no game is at, is
  ignored: it could otherwise put a forged world or forged results in.
- **[S6] the world check** (section 6): the world is read to its exact end against the match: the
  map, the roster, every player's kits.
- **[S7] the other games agree.** The server uses an upload as the match's world only when its
  CRC (over the whole body) is what most games reported for that tick in their 0xDF hashes, or
  another game reported the same CRC. One game alone cannot make its forged world the one others
  load. The exception is the initial world, which no other game can vouch for yet: it must be a
  world where nothing has happened, so it is refused if it has any kills or counted events, or any
  player in it (players enter with 0xE9 after the clock starts, so a real initial world has no
  player block).

What the receiving game checks before it loads a world it was sent (0xE8):

- **[C3] the envelope** (patch 114): the generation and the compressed flag, and that the LZMA2
  stream unpacks into the 0xF000-byte buffer and no further.
- **[C4] the same world check** (patch 87), with the context the game has: the map's rules, the
  batteries, the respawn points. It has no profiles, kits or map object list, so it reads the frame
  and every section but cannot read inventories and objects against the match.
- **[C5] the game's own guards** (patch 115): at the native readers that would index with a
  value from the world (a weapon's state machine, an anomaly's state, an artefact's id), an index
  outside its table takes the reader's own "none" path instead; a dead selector's death animation
  past its set's lists is read as the first (the lists' lengths are content, so no check before
  can know them); a double-barrel's or pistol's fire animation past the state's two is stored as
  the first; a throw's grenade past its set's grenades, or a set that is not a grenade set, is
  not ready and is not thrown (the set's size is content too); an animation a weapon state waits
  for past the state's own animation handles never ends. Real worlds carry such indices (the
  show state's 63, 205, 220...: LIKELY never set), and the native compare then reads what lies
  after the object, different in every game, so "no match" is what they already get.

A refused world is not loaded: the game tells the server (a "Rejected" receipt) and gets a fresh
one (patch 97), without leaving the match.

## 6. What a world contains, and how each part is read

```
 world body (at most 0xF000 bytes)
 +-------------------------------------------------------------+
 | u32 active mask        bit k set = place k has a block      |
 +-------------------------------------------------------------+
 | per set bit, by place:                                      |
 |   u16 length                                                |
 |   player state  (length bytes)                              |
 |   u8  animated  (0 or not)                                  |
 |   animation stream (a bit stream, when animated)            |
 +-------------------------------------------------------------+
 | six sections, each u16 length + its bytes:                  |
 |   bullets | statistics | game objects | physics | rules |   |
 |   effects                                                   |
 +-------------------------------------------------------------+
```

The check is one parser (`srp-validate`, over `world-frame`) shared by the server and the game.
Every record is parsed once into a typed view that borrows its bytes from the world. While it
parses, each field is turned into its type, and the world is refused if a field does not fit:

- a byte with a closed set of values becomes an enum (a trap's state, an artefact's state, a
  rule's phase), and any other value is refused;
- a bool must be one of the bytes the game writes, 0x00 or 0xFF;
- an index is bounded by the table it indexes (an anomaly's states, the map's artefacts, a gun's
  ammo list);
- a count is bounded by the buffer it fills (at most 20 users of an object, 8 affects of a body
  part);
- a float must be a number (`Finite`);
- the record must end exactly where the next one starts, and the world exactly at its end.

Then rules that need the match are checked over the parsed values: an item's owner must be a kit
of the match that has a player, a user of an object must be a place of the roster, an artefact's
id must be one of the map's.

The parts, and what they hold:

- **Player states**: for each place, its kit in use, its stored item amounts, life, look and
  input, effects, stamina, and its inventory: every item of the kit in slot order with its body (a
  weapon's state machine and recoil, a medkit's influences, a carried battery), then the damage
  model, the movement controller, recent hits and the carry snapshot (patch 78). The layout of an
  item depends on live state the world does not say (whether a gun has been in a hand, whose ammo
  a picked gun has), so the parser tries each layout the game could have written and takes the
  one that reads to the end.
- **Animation streams**: a bit stream per player: a tree of animation nodes, then a table of times
  and a table of floats that the tree refers to by index. Every index and every count is checked
  against its table, and the nesting of the expression tree is capped, since the game's reader
  recurses with no limit.
- **Bullets**: each bullet in flight: its shooter, gun slot, ammo, position and speed. Its shooter
  must be a place, and one with no block (a player who left with bullets still flying) must be a
  roster place with a gun in that slot in every kit, because a joining game looks the shooter's
  gun up.
- **Statistics, rules**: the scores, the batteries and their carriers, the time limit and the
  mode's rule; each field in its domain.
- **Game objects**: the grenades and traps of every place's kits, then the map's damage zones,
  resupply zones and anomalies with their artefacts, in the map's order.
- **Effects**: the effects playing, in the one form a game writes (an emitter by index, never by
  address).

Example, one field of the effects section of a live world:

```
 ... 01 ...        the effect's activation status at offset 17
     |
     +-- 0 created, 1 active, 2 stopped: parsed into an enum.
         A world with 3 here is refused, and the refusal names the field.
```

Two kinds of real worlds needed care (found on 2026-11-07 by checking every upload of every live
revision-50 match, 175,872 worlds):

- **A player whose state went NaN.** The game's movement controller made one player's position
  NaN (a server bot, just after it stood up from a crouch); its speed, its last position, its
  bullets' origin and some animation floats followed (ticket T117). Revision-50 games
  wrote those worlds, and the parser took the three fields as any f32 while they played.
  Exe patch 118 keeps the position finite, so these are refused again: every float a
  number, every animation floats index below its table's count.
- **A bullet whose shooter has left.** Allowed with the rule above.

## 7. When something is refused

```
 input refused    -> not relayed; the sender is told to drop it (0xF1/0xF2/0xF5)
                     bad inputs in a stretch -> the sender is removed

 world refused    -> sent to nobody, ever; the server asks another game
 (server)            the line in the log names the section, offset and reason
                     3 in a row from one game -> it is resynced, 3 more -> removed
                     a game waiting for a world, every source refused -> removed
                     no initial world but refused ones -> the match is not made

 world refused    -> not loaded; the game sends a Rejected receipt
 (game)              and gets a fresh world (patch 97), staying in the match
```

A refused world is never taken as proof of cheating: it may be the check that is wrong. But it
is never passed on either (owner, 2026-11-07: "the server can block, we don't want to pass bad
worlds around"; this replaced a temporary fall-back to the game's own check). The game whose
world was refused is asked last from then on, and every game that waits for a world gets an end
when none can be had, by counts, never by time (`packet-validation.md`, "A refused world is
never relayed"). The release's check of real recorded worlds (`srp-world-check`) is what keeps a
wrong check from reaching players.

## 8. The lobby: names, chat, clans

Outside a match, players reach each other through the lobby and its chat server, over TCP. The
server builds every message, but some of their text was typed by a player:

| Text | From | Checked |
|---|---|---|
| Player name | the account's login name | at most 99 bytes; cut to the game's 64-byte buffer (63 and a NUL) wherever it is sent |
| Chat line | a player's message | cut to the game's 256-byte buffer |
| Clan name | the clan's founder | only the characters of the game's own clan-name rule |
| Clan member name | each member's name | cut to the 32-byte field the game reads it into (31 and a NUL) |
| Clan daily message | the clan's leader | cut to the game's buffer |

The game copies each string into a fixed buffer without checking its length, so the server cuts
every one to its buffer before it sends it.

## 9. What is not checked yet

- **Player text inside the interface.** The chat movie escapes `<` in a chat line, but it puts
  the sender's name into its HTML as it is. A name with markup in it would be read as markup by
  the other players' chat. Whether that can do more than spoil the formatting has not been
  reviewed.
- **The match messages are not typed views yet.** Inputs, relays and kit choices are checked by
  the older byte-level code (`srp_validate::messages`). The checks are complete, but they are
  not yet parsed into types the way the world is.
- **A match where one game simulates alone.** A game that joins it loads that game's world, and
  no other game can vouch for it (S7). The world still has to pass the whole check.
