# Which weapon a kill, a shot and a hit were made with (research, 2026-11-02)

The owner wants per-weapon player stats on the site: the weapon of each kill, and shots fired and hit
per weapon. What the 0.26g game gives the server, and what a client patch would take. Static analysis
only (the original exe, the 0.21d PDB); nothing run in a game. Addresses are VAs (image base 0x10000).
Tags: PROVEN (read in the code at the address given), LIKELY, GUESS. Sections 1 to 3 are the game
without the patch; section 4 is the patch built from them (patch 165, 2026-11-20).

## 1. What the server gets today: no weapon

- **No message a client sends names a weapon** (PROVEN). The client->server match messages are
  0xD8..0xE1 (`match.md` 1.1): connect, port probe, ready, time reply, the inputs (`actions_mask`
  and rotation only), the kit choice (0xDE, one byte), the hash, leave and keep-alive. No kill
  feed, death record or hit/damage message exists in either direction (`match.md` 2.4).
- **The patched client's world upload** (0xE0 chunks, `match_msgs/upload.rs`; client patch 3.2) is
  the only state the server sees. In it:
  - the statistics blob counts the 28 events per place **per kit** (`events[28][3]`,
    `snapshot_rules.md` 3.1; `crates/survarium/src/v026g/match_msgs/snapshot.rs:294-296`): kill 0,
    headshot 15, melee 13, grenade kill 25, shots 22, hits 23, each by kit, not by weapon (PROVEN);
  - the damage record of a player is per inflictor and kit (the f32 sum 0x2F58E0 adds to, handler
    slot +18h, called at 0x2DA10A), with `last_hit_ms` and `last_hit_enemy_id`: no weapon (PROVEN);
  - each player block has the inventory: `m_active_slot` and every item with its slot and, for a
    weapon, its magazine (`snapshot_rules.md` 7.4). That is the state at the world's tick, a few
    seconds apart, not an event (PROVEN for the fields; the server reads only the supplies);
  - the bullet manager blob has the bullets in flight, each naming its weapon by the shooter's slot
    (`client_patch.md`, patch 85 "The bullets": bullet +254h written at 0x32C0B5). Transient: a
    bullet that hit is gone by the next world (LIKELY).
- **Where the server's "killer" comes from** (PROVEN): nowhere on the wire.
  `tdm_death_costs_ticket` (`crates/survarium/src/v026g/match_msgs/rules.rs:297`) is a model of the
  client's own Team Deathmatch rule (0x31614E..0x31618C), used by its tests only. The match server
  pairs killer and victim from two vouched worlds' statistics: when exactly one place gained kills
  and the deaths gained add up to them (`servers/match-server/src/v026g/kill_cap.rs:32-68`, called
  from `game.rs:3857`); kills and deaths per place come from the kd rule (`snapshot_rules.md` 2.7).
- **Replays** are the relayed inputs and uploaded worlds (`replay_sur.md` 4, 5): a client playing
  one computes every kill again, but nothing in the file names the weapon (PROVEN by the format).
- The kill feed's row is local hud only: the world's event node (+0 victim, +1 killer, +2 the
  weapon's `combat_log_icon`, +4 cause; `client_patch.md` 3.55) is played by the hud
  (0x23B5F0 -> `game_world_ui::on_player_killed` 0x237C70) and never sent (PROVEN). The icon is
  not an id either: ported weapons share stand-in icons (`kill-feed-icons.md`).

### What the server can do with no patch

- A kit has two weapon slots, 7 and 10 (the client's table 0x9266A0; `session/src/v026g/rules.rs:806`),
  and the static info names each kit's slot items (`static_info.md` 3.1). So the per-kit events
  are **exactly per weapon for a kit with one gun** (slot 10 empty): kills (event 0 less grenade
  kills 25; melee 13 is the gun's own butt, `[bullet+40h] == weapon+53Ch` at 0x2DA2CD), shots 22,
  hits 23 (grenades and traps never count: 3).
- A kit with two guns mixes them; a gun picked up off the ground (patch 85) is counted under the
  picker's kit. So the no-patch split is right for one-gun kits and a mix for the rest.

## 2. Where the client knows the weapon

The statistics handler (`base_game_statistics_handler`, vtable 0xA0755C, the same slot order as
0.21d's 0x814A1C) is told the weapon at all three events and drops it (PROVEN):

| event | caller -> handler slot | weapon in the call | the counter |
|---|---|---|---|
| kill | `base_player::generate_killed_event` 0x2DA170 -> slot +0 `on_player_killed` 0x26DA00 (17 stack args, `ret 44h`; call at 0x2DA46C) | arg 4: the damage source's **dict id** (`word [[source+25Ch]]`, 0x2DA3F7..0x2DA404); arg 13: the killer's **item instance id** (below); arg 17: the bullet | `shared_statistics::on_player_killed` 0x2F4EB0 (from 0x26DA3F) gets time, killer, killer kit, victim, victim kit, headshot, melee, carrier, the source's **category**, one more; not the id |
| hit | `base_player::generate_hit_event` 0x2D9F40 -> slot +4 `on_player_damaged` 0x26DA50 (10 args, `ret 28h`; call at 0x2DA164) | arg 7: the attacker's item instance id; the bullet is EBP in 0x2D9F40 | `shared_statistics::on_player_damaged` 0x2F5450 (event 23, from 0x26DA6C) gets attacker, kit, victim, a flag, melee |
| shot | `base_player::on_fire` 0x2D7800 -> slot +8 `on_player_weapon_firing` 0x26DA80 (5 args, `ret 14h`; call at 0x2D78A5) | arg 3: the shooter's item instance id | `shared_statistics::on_player_weapon_firing` 0x2F5500 (event 22, from 0x26DA91) gets place, kit, count |

- **The dict id** (PROVEN): an inventory item's +25Ch is its items-dictionary entry (`is_stack` at
  its +22Ah read at 0x2FD49A, as `static_info.md` 3.2 says); the entry's +0 is the word read from
  `dict_id` and +228h the byte read from `item_category` (the loader 0x2C8FBF and 0x2C90CD, strings
  0xA020EC, 0xA0EAE0). For a bullet the damage source is the weapon's inventory item: try_pierce
  passes `[bullet+0Ch]` (0x32D9E6) as `base_player::hit`'s last argument, which `hit_info::hit_info`
  stores at +40h (0x32BA47), which hit_impl passes on (0x2D9F03); the bullet's +0Ch is the weapon
  core + 10h (0x32BB12, the weapon from instant_fire's `mov edi,ebx` 0x2E2518). For a grenade or a
  trap it is the grenade's or trap's set, an inventory item (`[grenade+0Ch]` 0x2E85C4, `[trap+194h]` 0x2E6694,
  passed by `bullet_manager::explode` 0x33985C to `base_player::hit` 0x2D9A60, stored at hit +40h; 0x2D9AE0 is
  the anomalies' path, with no source) (PROVEN).
- **The instance id** (PROVEN): `profile [player+6B8h]`, kit `[player+115h]`, slot `[weapon item+254h]`
  (hit, kill: the bullet's weapon, 0x2D9F97, 0x2DA1EA) or the inventory's selected slot
  `[[player+10Ch]+170h]` (shot, 0x2D7854): `item = word [profile + 98h + (kit x 1C1h + slot) x 4]`
  (the kit's slot table, `static_info.md` 3.1), `id = dword [profile + 1568h + item x 0B4h]`, the
  lobby's item id. No bullet (a grenade, an anomaly): slot 17h, id 0. A picked-up gun (patch 85)
  sits in the picker's slot, so this id is the picker's own item, not the gun: the dict id is the
  one to key on.
- The kill's arg 3 and the shot's arg 4 (and `bullet+58h`) are `weapon_core::player_stance`
  0x2E43A0 at the shot (the bullet takes it at 0x32BAED): no weapon id (PROVEN).

## 3. How the client counts shots and hits

- **Shots** (PROVEN): `weapon_core::instant_fire` 0x2E23A0 fires the ammo's pellet count
  (`[[weapon+538h]+2ACh]`, 0x2E2467) of bullets (`bullet_manager::emit_bullet` 0x322830 each), then
  calls its user's +110h interface slot +4 (`[weapon+594h]`, 0x2E258D) with that count:
  `base_player::on_fire` 0x2D7800 (0.21d 0x1D3980, the same body) tells its fire listeners, then
  the handler, and 0x2F5500 adds the count to `events[22][kit]` (u16, `[record + kit x 2 + 84h]`).
  A shotgun's shot counts its pellets.
- **Hits** (PROVEN): every hit that took health (hit_impl, `hit_impl` row of `abi_inputs.tsv`) ends in
  `generate_hit_event` 0x2D9F40; its handler call reaches 0x2F5450, which adds 1 to
  `events[23][attacker kit]` (`[record + kit x 2 + 8Ah]`) when the attacker is a place, of the other
  team, the call's flag (0x2D9F40's 7th argument) is set and the hit is not melee. At most one hit a
  pellet. The flag is set only for a bullet's first damaging hit (`hit_impl` 0x2D9C84..0x2D9C9A: the hit
  source's vtable slot 0 gives a bullet and its `m_max_damage_dealt` +54h is still 0, which
  `hit_body_part` raises at 0x2E50EC). A grenade's or trap's hit (their source's slot 0 is 0x300040,
  `return 0`), an anomaly's (no attacker) and a bullet's later hits are never counted (PROVEN).
- **Kills** (PROVEN, `results.md` 9): 0x2F4EB0 counts event 0 under the killer's kit for an enemy,
  4 for a teammate, 3 for no killer or oneself, plus 15, 13, 10 and 25 (the category test at
  0x2F514E).
- **Per kit in memory, never per weapon** (PROVEN for the record: `events[28][3]` of u16 in each
  0x1F8-byte place record from `shared_statistics`+0, 20 places, then the battery manager at
  +2760h). The three handler thunks are the only sites that see the weapon, and they keep nothing.
  That no other object counts per weapon: PROVEN (the handler slots are only 0x26DA00 / 0x26DA50 /
  0x26DA80, vtables 0xA0755C and 0xA07FBC, both dropping the weapon; the statistics' saved fields are per
  preset, named by their writer strings 0x335750, 0x3353C0; the hud's only `weapon_stats` call is the ammo
  panel, `HUD.as:1024`).

## 4. The patch that reports it: patch 165, `weapon_stats` (built 2026-11-20)

A Rust patch in the arena (`rust_patches.md`; `client_patch.md` "Per-weapon counts (patch 165)"),
like patch 92: its state is the statistics' in every way and travels in the world after them, so a
rollback takes it back with the native counters and the match server reads it from a trusted world.
The table's type, its wire form and its domains are one definition,
`crates/srp-validate/src/world/weapons.rs`, used by the game (to keep, write and load it), by the
match server's check and the game's own (patch 87), and by the results' decoder. The wire layout
is `snapshot_rules.md` 3.5.

### 4.1 What is counted, per place and weapon

| field | where | the weapon | adds up to |
|---|---|---|---|
| kills | on_player_killed 0x26DA00, an enemy kill | the damage source's dict id (arg 4) | event 0, modulo 256 |
| headshots | the same call, arg 14 | the same | event 15, modulo 256 |
| wall_kills | the same call: the killing bullet (arg 17) changed course more often than it bounced | the same | at most kills |
| ricochet_kills | the same call: the bullet bounced | the same | at most kills |
| deaths | the same call, every one | what the victim holds | event 2, modulo 256 |
| shots | on_fire 0x2D7843, the count | what the shooter holds | event 22, modulo 65536 |
| hits | generate_hit_event 0x2DA10C, under on_player_damaged's tests | the bullet's weapon | event 23, modulo 65536 |
| damage | the same site: the pain lost (the frame's +30h) of an enemy's hit | the bullet's weapon | nothing (below) |
| kill_distance, longest_kill | on_player_killed: its two position arguments | the damage source's | nothing |
| held_ms | base_player::tick 0x2DAAD3: 50 ms a tick, the player alive | what the player holds | nothing |

- **The sums are exact by construction, not by a look at the numbers:** each count the statistics
  have too is made at the one call that makes theirs, under the same tests, and the table begins
  only where the records are zero or come from a world that has it. The modulus is the record's
  own: a kit's kills, headshots and deaths are a byte in a world (`snapshot_rules.md` 3.1), and a
  game that loaded a world has only those bytes; shots and hits are u16 and wrap.
- **The pain** (PROVEN for the argument, LIKELY for its unit): generate_hit_event's 5th argument
  is what the handler's slot +18h adds to the victim's `received_damage` for an attacker of
  another team (0x2DA0C6..0x2DA10A, 0x2F58E0); the client's override names it the pain the victim
  lost (`abi_inputs.tsv`, 0x1F3C70), and a pain pool's maximum is 1.0 (`bandages.md`). It is kept
  in ten-thousandths, a hit rounded to the nearest. The game's record is an f32 a victim, inflictor
  and kit: there is no native total these sums could equal. The stock's hits and hits the
  statistics count no event 23 for (the flag clear) add their pain too.
- **The distance** (PROVEN for the arguments, LIKELY metres): on_player_killed's 6th and 10th
  arguments are pointers to the killer's and the victim's positions, the translation of their
  transforms (0x2DA278..0x2DA2B4 a copy on the caller's stack, `lea edx,[esi+43Ch]` 0x2DA438). The
  game's positions are metres (`fall_damage.md`: heights of 5 and 10 with a gravity of 9.8; patch
  92's 10 m steps, played). Kept in decimetres, summed (a u32) and the longest (a u16, 6553.5 m at
  most). A killer with no player has zeros there (0x2DA28E): such a kill adds no distance. It is
  the distance at the death, not at the shot: a grenade's kill is from where its thrower stands
  then.
- **Through cover and by a ricochet** (PROVEN): a bullet's +84h is `m_change_trajectory_count`
  and its +88h `m_ricochet_count` (the names its serialize passes; `world/bullets.rs`).
  `change_trajectory` 0x32EE20 adds one to +84h and is called by try_pierce (0x32DE2C) and
  try_reflect (0x32E335) alone; try_reflect then adds one to +88h (0x32E33A, at most 2:
  0x32DE4D). So +88h not 0 is a bounce, and +84h over +88h a pierce: of a wall, a prop or another
  player's body (try_pierce has no material test). The victim's own body is not counted: try_pierce
  hits what it goes through before it changes course. A kill that pierced and bounced counts in
  both (the kill feed, patch 67, shows the ricochet alone). Not for a stock hit (arg 15).
- **What a player holds** (PROVEN for the reads): its held object (+128h) when one of its
  inventory's 23 slots holds it (a slot's item is the object + 10h: the bullet's constructor takes
  a weapon's item so, 0x32BB12), by that item's dictionary entry (+25Ch, read of every slot's item
  by update_weight 0x2FC71C). The held object is compared, never read: a battery, which no slot
  has, is no weapon known. During a switch the old weapon is held until the new one is (+12Ch the
  pending one). LIKELY that a grenade set or a medkit in the hands is its slot's item + 10h too;
  if not, it is counted as no weapon known.
- **The time held** is a per-tick count, not a stamp: `base_player::tick` runs once a simulation
  tick (50 ms, `match.md` section 1) for each player in the world (LIKELY: the tick list's call,
  the site of patches 92, 109 and 111); the hook adds 50 ms for a player whose `m_is_alive` (+33Ch)
  is set. A stamp at every way a held item changes (a switch, a spawn, a death, a battery, a
  pickup, a kit change) would need each of them proven; a tick's add is in the world with the
  rest, so a rollback's replayed ticks count once and a late joiner has what the others have. Its
  cost is one more handler on a site whose entry already saves the machine. The player's own tick
  delta (EBP at the site) is not used: +324h is this game's last tick of the player, which a load
  does not give.
- **No weapon known** (dict id 0): a hit and its pain with no bullet (a grenade, a trap: the hit
  event carries the bullet, not the source), a kill with no source, a death, a shot or a tick
  with nothing of the inventory in the hands, and a place's weapons past its fifteenth.

### 4.2 The sites, each re-read in the exe

| site | VA | span | what the proposal said, re-read |
|---|---|---|---|
| `weapon_stats.killed` | 0x26DA00 | 9 | Held. The frame also carries the victim's kit (+30h), the killer's (+20h), the headshot (+38h), the melee byte (+3Ch), the two positions (+18h, +28h) and the bullet (+44h). The victim's player is the handler's core's place's (handler +3238h, as 0x2DA183). |
| `weapon_stats.hit` | 0x2DA10C | 6 | Held (ESI, EBX, EBP, +24h, +34h, +38h on every path to it). More: EDI is the attacker's player or 0, and the kit the statistics count the hit under is that player's, 0 with none (0x2DA127); +30h is the pain. |
| `weapon_stats.fired` | 0x2D7843 | 9 | Held. The weapon is taken as what the shooter holds (above), not as `[esi+18h]`'s +26Ch read blind: the same value for a weapon. |
| `weapon_stats.save` | 0x2F5BD0 | 5 | Held. One append (`buffer_writer::w` 0x7AC10) writes the block, not a byte writer's call a byte. |
| `weapon_stats.load` | 0x2F5C83 | 7 | Held. The reader is the statistics section alone (0x2D2AEE..0x2D2B09), so what follows the tail is the block. |
| `weapon_stats.reset` | 0x2F4C31 | 6 | Held; the reset also zeroes the options (+31C8h, 0x2F4CDA) until the next start_match, and runs only when the client's option 0xAA2FF0 says so (0x27B4C9, 0x218CE8). |
| `weapon_stats.made` | 0x2F4BC1 | 6 | New: the statistics' constructor (0x2F4B40), the other place their records are zero. Without it a game's first match would be uncounted. |
| `battery.carrier_tick` | 0x2DAAD3 | 5 | The existing site (order 5), for the time held. |

- **The callers** (PROVEN: each of the 35 loads of core+18FA8h read): the handler's slot +0 is
  called at 0x2DA46C alone, +4 at 0x2DA164, +8 at 0x2D78A5, +18h at 0x2DA10A, +30h (serialize) at
  0x2D265F, +34h at 0x2D2B18. No patch of ours counts events 0, 2, 15, 22 or 23.
- **Twenty places, always:** a patched game's serialize and deserialize walk 20 records whatever
  the match's count (`STATISTICS_COUNT`, `patch_026g.py`), and so does the block.
- **The proposal's wire** put the block's size after it, for a reader coming from the end. That
  cannot tell a block from a tail: a tail's last word is the high half of
  `m_active_in_match_players_mask`, which nothing writes in modes 2 and 3. The length follows the
  version byte instead and must be the bytes left, and every reader walks the records from the
  front (`StatisticsTail::of`, which no code used, is gone).

### 4.3 Counted or not: the states

- `Counted`: from the statistics' constructor, from their reset, and from the load of a world with
  the block. The hooks count and the save writes the block.
- `Uncounted`: after the load of a world without the block (or with one that does not read), until
  the next reset. The hooks count nothing and the save writes nothing, so this game's worlds are
  that world's game's byte for byte.
- So a block exists only where the counting was unbroken since the records were zero: the sums'
  rule can hold in every world that has one.
- **Who must have it:** the world's bytes and hash change, so every game of a match runs the same
  revision (`SRP_MIN_PATCH_REVISION`), as with patch 92. A match's first world is a game's upload
  (`match.md`, "The initial world's time"): of a patched game, with the block.
- **No `simulation_changes.tsv` row:** nothing a match plays out changes. A replay of an older game
  loads that game's first world (no block), is uncounted from then on, and writes the worlds the
  recording has.

### 4.4 The server

- `srp_validate::world::Statistics::read` reads the block after the tail to the section's exact
  end and refuses a world whose domains or sums do not hold (`snapshot_rules.md` 3.5); with the
  match server's context, a dict id that is no item's. The game's own check (patch 87) is the same
  code.
- `snapshot::Statistics::weapons` (by place; `None` without a block) and
  `matches::PlayerResult::weapons` / `rewards::PlayerResult::weapons`: the last trusted world's
  `snapshot::WeaponCounts` of the place, empty when no world was trusted or its games count none.
  A dict id's name is the game data's (`Dictionary::item`), never a table in code.

### 4.5 Not run in a game: what a local test match must show

- Two patched games play a match to its end with no resync that a match without the patch has
  not: the block is the same in both games' worlds (the server's hashes agree), through a
  rollback and a late join.
- No upload is refused for the weapons block (the server's log, the games' refusal log): the
  sums' rule holds on real play, with a kit change, a picked-up gun (patch 85), a grenade and a
  trap kill, a stock hit, a battery carried, a team kill, a suicide and a fall.
- The result's `weapons` of a player are what he did: the weapons' dict ids are his guns' (a
  picked-up gun under its own id), the shots a magazine's, a shotgun's by pellets; a grenade's
  kill under the grenade's id.
- The distances are plausible metres (a kill across a known map distance), the damage is about
  1.0 a kill's worth (10000), the time held is about the time alive.
- A kill through a wall and one by a ricochet (the kill feed's icons name them) count.
- What is in the hands with a grenade, a medkit and a battery reads as written above.
