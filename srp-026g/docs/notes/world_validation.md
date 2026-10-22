# World validation: what a game trusts in a world, and what the server checks

**Version scope:** client **0.26g (build 2777, no PDB)**. Addresses are VAs in
the original exe (image base 0x10000). Tags: **PROVEN** (read at the address
given), **LIKELY**, and **observed, not traced** (a bound taken from real
worlds because the exe gives none or it was not read).

Code: `crates/survarium/src/v026g/match_msgs/world.rs` (`world::validate`),
called by the match server for every upload (`game.rs`, `Game::uploaded`).
Tool: `servers/match-server/examples/world_check.rs`. Stacked on PR #282
(match.md section 36: the frame, the section caps, the mask bound, the
vouched `recent`).

## 1. The rule

- A game reads a world (0xE8) with no check of its own, and whatever one
  client uploads, every game started from it loads (match.md section 36).
- So the server reads every upload first. It refuses a world when it
  cannot read a part to the part's exact end, or a value the game trusts
  is outside the bound the game needs.
- **Strict on purpose (the owner, 2026-10-22):** what is not fully
  understood or bounded is refused, at once, with no log-only phase. It
  can be loosened later.
- **Loud:** each refused world is
  - one warning line: the match, the player, the time, the size, and
    `<part> at <offset>: <reason>`;
  - a dump (kind `unloadable`, in `SRP_SNAPSHOT_DIR`);
  - a count, `uploads_refused`, in the player's summary line ("N uploads
    no game could load").
- A refused world goes to nobody: not the initial world, not a
  candidate, not `recent`, not a world to look at. Its client stays asked,
  so a world of its that reads can still come.

## 2. What is read, in the order a game applies a world

World apply `game_world_core::deserialize` 0x2D2770 (the second body of
that name; the first, 0x2D0B70, only copies the sections into a history
item, 0x324D80):

```
  u32 time | u32 mask | player blocks ... | u16+bullets | u16+statistics
  | u16+objects | u16+physics | u16+rules | u16+effects
             |                     |              |                 |
             v                     v              v                 v
       per set bit k:        bullet_manager   statistics        rules: each rule's
       place k's head  ->    ::deserialize    (vtbl +0x34)      vtbl +0x40, then the
       kit objects           0x321C50                           blackboard 0x323DC0
```

| Part | Check | Code | Status |
|---|---|---|---|
| frame | walks from the front; no mask bit past the roster; caps | `world::frame` | done (PR #282) |
| player head | kit bytes, bools, floats | `world::player_head` | done |
| statistics | reads to its end; places and numbers | `world::statistics` | done |
| rules | reads to its end; places, batteries, points | `world::rules` | done |
| physics | empty | `world::physics` | done |
| effects | | | next |
| bullets | | | next |
| player state past its head | | | next |
| game objects | | | next |

## 3. The player block's head

Layout (snapshot_rules.md section 7.3; M = the place's profile item count):

```
 offset  0        1        2            2+4M      +1      +2      +3       +7
        +--------+--------+------------+---------+-------+-------+--------+--------+--
        | current| target | M x u32    | alive   | has_to| select| pitch  | rot y  | ..
        | kit    | kit    | items      | bool    | _die  | anim  | f32    | f32    |
        +--------+--------+------------+---------+-------+-------+--------+--------+--
```

**What the reader trusts:**
- `current_index` (byte 0). World apply 0x2D2770 compares it with the
  slot's current kit (`[entry+0xC]`, 0x2D2A3A), and calls the kit's
  player object through its vtable +0x3C. The object is
  `[entry + kit*4]` with no bound (0x2D2A85..0x2D2AA0), where the entry is
  the place's in the table at `[core+0x18458]` (stride 0x30). The table has
  three entries, one for each kit, and only the kits sent as usable
  (`KIT_USABLE`) have an object; the others are null. PROVEN.
- `target_index` (byte 1), the next kit. It is stored at `[entry+0xD]`
  with no check (0x2D2A30..0x2D2A37). The next spawn
  (`player_respawn_rule_base::spawn_player` 0x316DA0) copies it to
  `[entry+0xC]` (0x316DDA..0x316DE3) and calls through
  `[entry + kit*4]`, again with no bound (0x316DF2..0x316DF9). PROVEN.
- The kit choice of a relayed profile input is checked (0x321790: under 3
  and an object, match.md section 23). The same byte in a world is not.
- So a world with a kit byte of 3 or more, or of a kit with no object,
  makes every game that loads it call through memory past the table, or
  through null, at once (current) or at that player's next spawn (target).

**The bound the server checks** (`world::player_head`):
- both kit bytes `< 3`, and the place's profile has that kit with state
  `KIT_USABLE` (the server sent the profiles, `formed.profiles`);
- a block of a place with no profile is refused;
- the head reads for the profile's M items (`PlayerState::read`);
- the three bools are 0x00 or 0xFF (what the bool writer 0x80FA0 writes);
- the look pitch and the rotation are finite. Observed, not traced: what
  the game does with a non-finite one.
- The kit table has three entries: a place has up to three player
  objects, one for each kit (static_info.md section 3.1), and the profile
  input's check of 0x321790 takes a kit under 3 only (match.md section 23).

## 4. The statistics

**What the reader trusts** (`shared_statistics`, snapshot_rules.md
section 3): a record for each roster place, one for each battery in modes
2 and 3, then the 11-byte tail. The records are masked: a mask's bits
say what follows (`read_masked_f32`, the events), and the received-damage
masks name inflictors by place.

**The bound the server checks** (`world::statistics`):
- the section is not empty. Observed, not traced: `game_world_core::
  serialize` skips it when the statistics pointer is null (0x2D2644), and
  no kept world has an empty one;
- `Statistics::decode(bytes, places, batteries)` reads to exactly the end
  (batteries: the map's in modes 2 and 3, none in mode 4);
- every damage value and the newbie bonus is finite;
- `last_hit_enemy_id` is a place of the roster, or 0xFF. Observed, not
  traced: what reads it.

## 5. The rules

**What the reader trusts** (snapshot_rules.md section 2): the rules are
read in the order of the mode's config (timelimit, respawn, the mode's
rule, kd stats), then the two blackboard flags. Each rule reads its own
bytes; the section has no inner lengths.

**The bound the server checks** (`world::rules`):
- `Rules::decode` reads every part to the section's exact end: the
  timelimit with `m_current_time_in_ms` the world's time and no place
  past the roster in its mask; the respawn rule; the gather rule (modes 2
  and 3, read to exactly the kd stats) or the 12-byte tdm rule (mode 4);
  kd stats for every place; the two flags;
- a respawn rule and the mode's rule are both there;
- the respawn points are the map's (`MapMode::respawn_points`);
- no respawn time for a place past the roster;
- mode 4: the tdm rule's alive and active masks name no place past the
  roster;
- modes 2 and 3: the map's batteries (mode 3: those placed so far), each
  free battery's position finite, no container user past the roster, no
  battery past the map's in a container; the team points add up to no
  more than the containers hold, and each battery in a container says so
  (`Gather::read`).
- **Loosened 2026-10-22:** the points and what the containers hold were
  required to be equal. 19 of 20,167 worlds of a live sample were refused
  by it: every one late in one of five mode 3 matches that ended on the
  time limit at 9 to 0, 7 of them vouched for by two games. Each has all
  10 batteries in the winner's container (`ff 01` for each, the container
  listing all ten) and team points of 9. Why the tenth is in and not
  counted was not traced [LIKELY the rule's completion, match.md section
  26: once `+0x15C` is set the handler counts no put]. The points index
  nothing; the batteries and containers still are checked one by one.
  Test `a_container_may_hold_more_batteries_than_its_team_has_points_not_fewer`.

## 6. The physics

**What the reader trusts:** nothing; it reads nothing.
- The world's physics object has two virtuals for the section:
  - slot +0x40: `game_world_core::serialize` 0x2D2570 calls it with the
    section's writer (0x2D26C7..0x2D26D9);
  - slot +0x44: world apply 0x2D2770 calls it with the section's reader
    (0x2D2BEE..0x2D2C18).
- Both are on `[core+0x18FA4]`, which the core's constructor 0x2CFE90
  sets from one of its arguments (0x2D00AC).
- `vostok::physics::bullet_physics_world` (vtable 0x9E2EBC, written by its
  constructor 0x74950) is the only physics world 0.26g has. In its vtable
  the two slots sit between `move` (+0x3C) and `ray_test` (+0x48), new
  since the engine source (whose `physics::world` has no such virtuals).
  They are `ret 4` (0x75AF0) and `ret 8` (0x15A4B0). PROVEN for the
  vtable; that the core's object is of that class is LIKELY (the only
  implementation).
- So a game writes no physics bytes and reads none.

**The bound the server checks** (`world::physics`): the section is empty.
Any byte there is not a game's. All 2319 kept worlds have 0 bytes.

The scripted test client's ballast went into the physics past the
bullets' cap; it now goes into the game objects (to be made a real
section's contents when the objects are checked).

## 7. Proof against kept worlds

`world_check` over the five local sets (private/local-clients/runtime/
snapshots, private/fov-rev11/snapshots, private/farming-evidence-20261020,
private/editor-026, private/fov-slider), 2026-10-22: **2319 worlds, 2319
pass**: 2152 checked, 41 initial, 123 unchecked, 3 wrong. The owner's
live sample (20,167 worlds, run from the main session): 20,148 passed
before the loosening of section 5; the 19 refused pass since, read with
their real map, mode and roster. 1433 of them
had their match (map, mode, roster) from the server logs; 886 were
checked with the first match that lets them read. Without profiles the
tool checks the kit bytes against the table's three entries only.

```
cargo run --release -p match-server --example world_check -- \
    [-v] [--log server.log]... DIR...
```

It prints one `REFUSED <file>: <part> at <offset>: <reason>` line for
each refused world, then the counts by dump kind and by reason. With
`--log` it tries the logged roster and the 20 places of a growing roster;
a refusal is the logged roster's (until 2026-10-22 it was the 20 places',
which said "20 players" of a match of 3).

## 8. Stats the server could log to learn the real ranges

For the bounds marked "observed, not traced", the owner offered to
collect stats. Proposed, one line per match at its end:
- the largest look pitch and rotation seen in a head;
- the `last_hit_enemy_id` values seen;
- whether any statistics section was empty.

## 9. Tests

`world.rs` tests: `a_world_that_reads_in_every_mode_goes`,
`a_kit_byte_names_a_usable_kit_of_the_place`,
`a_head_with_a_bool_that_is_no_bool_or_a_float_that_is_no_number_goes_to_nobody`,
`a_place_with_no_profile_or_a_state_too_short_for_its_profile_goes_to_nobody`,
`statistics_must_read_to_their_end`,
`statistics_name_no_place_past_the_roster_and_hold_numbers`,
`rules_of_another_time_or_with_other_respawn_points_go_to_nobody`,
`rules_name_no_place_or_battery_the_match_does_not_have`,
`a_container_may_hold_more_batteries_than_its_team_has_points_not_fewer`,
`a_physics_section_with_anything_in_it_goes_to_nobody`,
`a_refusal_says_where`. The match server's `a_world_no_game_could_load_goes_to_nobody`
(the frame) stays as PR #282 has it.

Test fixtures that made worlds no game could have made were put right:
team points of 10 and 3 with 10 batteries (13 batteries home; now 10 and
0), and a bot's state with three items for a profile of one.
